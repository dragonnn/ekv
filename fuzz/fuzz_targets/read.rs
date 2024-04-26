#![no_main]
use ekv::flash::MemFlash;
use ekv::{Bound, Config, Database};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| fuzz(data));

fn fuzz(data: &[u8]) {
    if std::env::var_os("RUST_LOG").is_some() {
        env_logger::init();
    }
    let dump = std::env::var("DUMP") == Ok("1".to_string());

    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(fuzz_inner(data, dump))
}

async fn fuzz_inner(data: &[u8], dump: bool) {
    let mut f = MemFlash::new();
    let n = f.data.len().min(data.len());
    f.data[..n].copy_from_slice(&data[..n]);

    let config = Config::default();
    let db = Database::<_, NoopRawMutex>::new(&mut f, config);

    if dump {
        db.dump().await;
    }

    let mut buf = [0; 64];
    let rtx = db.read_transaction().await;
    _ = rtx.read(b"foo", &mut buf).await;
    drop(rtx);

    let rtx = db.read_transaction().await;
    if let Ok(mut cursor) = rtx.read_all().await {
        let mut kbuf = [0; 64];
        let mut vbuf = [0; 64];
        while let Ok(Some((klen, vlen))) = cursor.next(&mut kbuf, &mut vbuf).await {
            assert!(klen <= kbuf.len());
            assert!(vlen <= vbuf.len());
        }
    }
    drop(rtx);

    for _ in 0..100 {
        let mut wtx = db.write_transaction().await;
        _ = wtx.write(b"foo", b"blah").await;
        _ = wtx.commit().await;
    }

    let rtx = db.read_transaction().await;
    _ = rtx.read(b"foo", &mut buf).await;
    drop(rtx);

    let rtx = db.read_transaction().await;
    if let Ok(mut cursor) = rtx
        .read_range(
            Some(Bound {
                key: b"foo",
                allow_equal: false,
            }),
            Some(Bound {
                key: b"poo",
                allow_equal: false,
            }),
        )
        .await
    {
        let mut kbuf = [0; 64];
        let mut vbuf = [0; 64];
        while let Ok(Some((klen, vlen))) = cursor.next(&mut kbuf, &mut vbuf).await {
            assert!(klen <= kbuf.len());
            assert!(vlen <= vbuf.len());
        }
    }
    drop(rtx);
}
