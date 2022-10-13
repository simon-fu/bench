/*
    use iftop to inspect bandwidth
*/

use tokio::{net::{TcpListener, TcpStream}, io::AsyncWriteExt};

const SERVER_ADDR: &str = "127.0.0.1:12345";

#[tokio::main]
async fn main() {
    tokio::spawn(async move {
        server().await;
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    //for _ in 1..100 {
        //tokio::spawn(async move {
            let mut cli = TcpStream::connect(SERVER_ADDR).await.unwrap();
            println!("connected to {}, start sending", SERVER_ADDR);
            let mut buf = [0u8;1024];
            while let Ok(_) = cli.writable().await {
                cli.write(&mut buf).await.unwrap();
            }
        //});
    //}
    //tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
}

async fn server() {
    
    let listener = TcpListener::bind(SERVER_ADDR).await.unwrap();

    println!("Listening at {}", SERVER_ADDR);

    loop {
        let (socket, _) = listener.accept().await.unwrap();
        // Clone the handle to the hash map.

        tokio::spawn(async move {
            let mut buf = [0u8;1024];
            while let Ok(_) = socket.readable().await {
                let _ = socket.try_read(&mut buf);
            }
        });
    }
}
