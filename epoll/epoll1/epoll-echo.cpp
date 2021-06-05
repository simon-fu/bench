

// from https://cloud.tencent.com/developer/article/1694517

#include <string>
#include <netdb.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <sys/epoll.h>
#include <stdio.h>
#include <errno.h>
#include <unistd.h>
#include <string.h>
#include <errno.h>
#include <fcntl.h>
#include <iostream>
#include <unordered_map>

#define dbgd(FMT, ARGS...) do{  printf("|%7s|D| " FMT, "main", ##ARGS); printf("\n"); fflush(stdout); }while(0)
#define dbgi(FMT, ARGS...) do{  printf("|%7s|I| " FMT, "main", ##ARGS); printf("\n"); fflush(stdout); }while(0)
#define dbge(FMT, ARGS...) do{  printf("|%7s|E| " FMT, "main", ##ARGS); printf("\n"); fflush(stdout); }while(0)


#define MAX_PENDING 1024
#define BUFFER_SIZE 1024

class Handler {
public:
	virtual ~Handler() {}
	virtual int handle(epoll_event e) = 0;
};

/**
 * epoll 事件轮询
 */ 
class IOLoop {
public:
	static IOLoop *Instance()
	{
		static IOLoop instance;
		return &instance;
	}
	~IOLoop() 
	{
		for (auto it : handlers_) {
			delete it.second;
		}
	}

	void start()
	{
		const uint64_t MAX_EVENTS = 10;
		epoll_event events[MAX_EVENTS];
		while (true)
		{
			int nfds = epoll_wait(epfd_, events, MAX_EVENTS, -1/*Timeout*/);
			for (int i = 0; i < nfds; ++i) {
				int fd = events[i].data.fd;
				Handler* handler = handlers_[fd];
				handler->handle(events[i]);
			}
		}
	}

	void addHandler(int fd, Handler* handler, unsigned int events)
	{
		handlers_[fd] = handler;
		epoll_event e;
		e.data.fd = fd;
		e.events = events;

		if (epoll_ctl(epfd_, EPOLL_CTL_ADD, fd, &e) < 0) {
			dbge("Failed to insert handler to epoll");
		}
	}

	void modifyHandler(int fd, unsigned int events) 
	{
		struct epoll_event event;
		event.events = events;
		event.data.fd = fd;
		epoll_ctl(epfd_, EPOLL_CTL_MOD, fd, &event);
	}

	void removeHandler(int fd) 
	{
		auto it = handlers_.find(fd);
		if(it != handlers_.end()){
			Handler* handler = it->second;
			handlers_.erase(it);
			delete handler;
			epoll_ctl(epfd_, EPOLL_CTL_DEL, fd, NULL);
		}

		// Handler* handler = handlers_[fd];
		// handlers_.erase(fd);
		// delete handler;
		// //将fd从epoll堆删除
		// epoll_ctl(epfd_, EPOLL_CTL_DEL, fd, NULL);
	}

private:
	IOLoop()
	{
		epfd_ = epoll_create1(0);  //flag=0 等价于epll_craete
		if (epfd_ < 0) {
			dbge("Failed to create epoll");
			exit(1);
		}
	}

private:
	int epfd_;
	std::unordered_map<int, Handler*> handlers_;
};

class EchoHandler : public Handler {
public:
	EchoHandler() {}
	virtual int handle(epoll_event e) override
	{
		int fd = e.data.fd;
		if (e.events & EPOLLHUP) {
			IOLoop::Instance()->removeHandler(fd);
			return -1;
		}
		
		if (e.events & EPOLLERR) {
			return -1;
		}
		
		if (e.events & EPOLLOUT)
		{
			bool broken = false;
			while (received > written)
			{
				ssize_t ret = send(fd, buffer+written, received-written, 0);
				if (ret > 0) {
					//dbgd("written bytes %d", ret);
					written += ret;

				} else if (ret == 0){
					//dbgi("writing disconnect, fd = %d",fd);
					broken = true;
					break;
				} else {
					int error = errno;
					if(error == EWOULDBLOCK || error == EAGAIN){
						// try again later
					} else {
						dbge("fail to writing socket, errno=[%d]-[%s]", error, strerror(error));	
						broken = true;
					}
					break;
				}
			}

			if(received == written){
				received = 0;
				written = 0;
			}

			checkWatch(fd, broken);
			
		}
		
		if (e.events & EPOLLIN)
		{
			bool broken = false;
			while(received < BUFFER_SIZE)
			{
				ssize_t ret = recv(fd, buffer+received, BUFFER_SIZE-received, 0);
				
				if (ret > 0) {
					//dbgd("read bytes %d", ret);
					received += ret;
					//buffer[received] = 0;

				} else if (ret == 0){
					//dbgi("reading disconnect, fd = %d",fd);
					broken = true;
					break;
				} else {
					int error = errno;
					if(error == EWOULDBLOCK || error == EAGAIN){
						// try again later
					} else {
						dbge("fail to reading socket, errno=[%d]-[%s]", error, strerror(error));	
						broken = true;
					}
					break;
				}
			}

			checkWatch(fd, broken);
		}

		return 0;
	}

	void checkWatch(int fd, bool broken){
		if(broken){
			IOLoop::Instance()->removeHandler(fd);
		} else {
			unsigned int events = 0;
			if(received < BUFFER_SIZE){
				events |= EPOLLIN;
			}

			if(written < received){
				events |= EPOLLOUT;	
			}
			IOLoop::Instance()->modifyHandler(fd, events);
		}
	}

private:
	ssize_t received = 0;
	ssize_t written = 0;
	char buffer[BUFFER_SIZE];
};

class ServerHandler : public Handler {
public:
	ServerHandler(int port)
	{
		int fd;
		struct sockaddr_in addr;
		memset(&addr, 0, sizeof(addr));

		if ((fd = socket(PF_INET, SOCK_STREAM, IPPROTO_TCP)) < 0)
		{
			dbge("Failed to create server socket");
			exit(1);
		}

		addr.sin_family = AF_INET;
		addr.sin_addr.s_addr = htonl(INADDR_ANY);
		addr.sin_port = htons(port);

		if (bind(fd, (struct sockaddr*)&addr, sizeof(addr)) < 0)
		{
			dbge("Failed to bind server socket, port %d", port);
			exit(1);
		}

		if (listen(fd, MAX_PENDING) < 0)
		{
			dbge("Failed to listen on server socket, port %d", port);
			exit(1);
		}
		setnonblocking(fd);

		IOLoop::Instance()->addHandler(fd, this, EPOLLIN);

		dbgi("listening on por %d", port);
	}

	virtual int handle(epoll_event e) override
	{
		int fd = e.data.fd;
		struct sockaddr_in client_addr;
		socklen_t ca_len = sizeof(client_addr);

		int client = accept(fd, (struct sockaddr*)&client_addr, &ca_len);

		if (client < 0)
		{
			dbge("Error accepting connection");
			return -1;
		}

		//std::cout << "accept connected: " << inet_ntoa(client_addr.sin_addr) << std::endl;
		Handler* clientHandler = new EchoHandler();
		IOLoop::Instance()->addHandler(client, clientHandler, EPOLLIN | EPOLLOUT | EPOLLHUP | EPOLLERR);
		return 0;
	}

private:
	void setnonblocking(int fd)
	{
		int flags = fcntl(fd, F_GETFL, 0);
		fcntl(fd, F_SETFL, flags | O_NONBLOCK);
	}
};

int main(int argc, char** argv) {
	int port = 7000;

	ServerHandler serverhandler(port);
	IOLoop::Instance()->start();
	return 0;
}

