

use bytes::Bytes;
use tokio::{sync::mpsc, time::Instant};
use tracing::info;



// env RUST_LOG=debug cargo test --release -- --nocapture
#[tokio::test]
async fn test_mpsc_throughput(){
    let packet_len = 51200;
    let max_packets:u64 = 1*1000*1000*2 ;
    tracing_subscriber::fmt().init();

    enum ChEvent {
        Data { packet : Bytes},
        Length { packet : usize},
    }

    // let mut packet = Cursor::new(vec![0u8; packet_len]);
    //let mut buf = BytesMut::with_capacity(packet_len);
    let packet = Bytes::from(vec![0u8; packet_len]);

    let (tx, mut rx) = mpsc::channel(16);
    let handle= tokio::spawn(async move {
        let mut npkt:u64 = 0;
        let mut nbytes:u64 = 0;
        let time = Instant::now();
        while npkt < max_packets {
            let r = rx.recv().await;
            match r {
                None => {break;},
                Some(ev) => {
                    npkt +=1;
                    match ev {
                        ChEvent::Data { packet } => {
                            nbytes += packet.len() as u64;
                        },
                        ChEvent::Length { packet } => {
                            nbytes += packet as u64;
                        },
                    }
                },
            }
            
        }
        let ms = time.elapsed().as_millis() as u64;
        info!("recv packets {}, elapsed {} ms, {} q/s, {} MB/s",
            npkt, ms, 
            1000*npkt as u64/ms,
            1000*nbytes as u64/ms/1000/1000);
    });

    let mut npkt = 0;
    let time = Instant::now();
    while npkt < max_packets {
        // let r = tx.send(ChEvent::Data{packet:packet.clone()}).await;
        let r = tx.send(ChEvent::Length{packet:packet_len}).await;
        if r.is_err(){
            break;
        }
        npkt +=1;
    }
    let ms = time.elapsed().as_millis() as u64;
    info!("send packets {}, elapsed {} ms, {} q/s, {} MB/s",
            npkt, ms, 
            1000*npkt as u64/ms,
            1000*npkt*packet_len as u64/ms/1000/1000);
    let _ = handle.await;
}

