use service::Server;
use std::env;

/// Run the server of the compression service on the address provided via the
/// commandline or the default address of 127.0.0.4000
#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:4000".to_string());

    Server::new_with_url(&addr).await?.serve().await
}

// TODO:
// Handle shutdown timeout by creating custom runtime and passing to `serve`,
// Here is an illustration,
// async fn main() -> Result<(), std::io::Error> {
//     let mut runtime = Runtime::new().unwrap();
//     let handle = runtime.handle();
//     runtime.block_on(async move {
// 	    Server::new_with_url("127.0.0.1:4001").await?.serve_with_runtime(handle).await
//     })?;
//     runtime.shutdown_timeout(Duration::from_millis(10));
//     Ok(())
// }
