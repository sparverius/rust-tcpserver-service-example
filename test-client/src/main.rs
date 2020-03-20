use std::env;

mod client;
use client::*;

use message::{Request, Response};
use service::message;

/// Currently can only verify GetStats responses with single client
const IS_CONCURRENT: bool = true;
const OVERLOAD_SERVER: bool = false;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:4000".to_string());

    run_clients(addr, 1000).await?;

    println!("Tests Complete");
    Ok(())
}

async fn run_clients(addr: String, num_clients: usize) -> Result<(), std::io::Error> {
    futures::future::join_all(
        (1..num_clients).map(|client_num| {
	    let the_addr = addr.clone();
	    tokio::spawn(async move { create_client(the_addr, client_num).await })
	}),
    )
    .await;
    Ok(())
}

/// Create a single client at the given address `addr`
/// For multiple clients,
async fn create_client(addr: String, client_num: usize) -> Result<(), std::io::Error> {
    println!("Starting Client {}", client_num);
    Client::new_with_url(addr)
        .await?
        .run_with(client_num, test_cases())
        .await
}

pub fn test_cases() -> Vec<Test> {
    if OVERLOAD_SERVER {
        flood_server()
    } else {
        cases()
    }
}

fn cases() -> Vec<Test> {
    let mut res = Vec::new();

    res.push(test_compress_ok(b"a", b"a"));
    res.push(test_compress_ok(b"aa", b"aa"));
    res.push(test_compress_ok(b"aa", b"aa"));
    res.push(test_compress_ok(b"aaa", b"3a"));
    res.push(test_compress_ok(b"aaaaabbb", b"5a3b"));
    res.push(test_compress_ok(b"aaaaabbbbbbaaabb", b"5a6b3abb"));
    res.push(test_compress_ok(b"abcdefg", b"abcdefg"));
    res.push(test_compress_ok(b"aaaccddddhhhhi", b"3acc4d4hi"));

    res.push(test_compress_fail_default(b"123"));
    res.push(test_compress_fail_default(b"abCD"));
    res.push(test_compress_fail_default(b"aaaaaaaaaaaaaaaaaaaaaaaaaB"));

    {
        if !OVERLOAD_SERVER {
            let msg = [97u8; ((message::MAX_PAYLOAD as usize) + 12)];
            res.push(test_compress_fail(&msg, Response::MessageTooLarge));
        }
    }

    res.push(Test {
        query_kind: Request::Ping,
        query: [97u8; 7].to_vec(),
        expected: Test::response_fail(Response::MessageTooSmall),
        validity: TestKind::Invalid,
    });

    res.push(Test {
        query_kind: Request::Ping,
        query: Test::header_bytes(0, 0, 1),
        expected: Test::response_fail(Response::MessageHeaderHasBadMagic),
        validity: TestKind::Invalid,
    });

    res.push(Test {
        query_kind: Request::Compress,
        query: Test::header_bytes(message::MAGIC, 0, Request::Compress as u16),
        expected: Test::response_fail(Response::CompressionRequestRequiresNonZeroLength),
        validity: TestKind::Invalid,
    });

    {
        if !IS_CONCURRENT {
            res.push(Test {
                query_kind: Request::GetStats,
                query: Test::request_get_stats(),
                expected: vec![],
                validity: TestKind::Valid,
            });
        }
    }

    // Note: will fail if resopnse is not Response::Ok
    res.push(Test {
        query_kind: Request::Ping,
        query: Test::request_ping(),
        expected: Test::response_ping(),
        validity: TestKind::Valid,
    });

    res.push(Test {
        query_kind: Request::ResetStats,
        query: Test::request_reset_stats(),
        expected: Test::response_reset_stats(),
        validity: TestKind::Valid,
    });

    {
        if !IS_CONCURRENT {
            res.push(Test {
                query_kind: Request::GetStats,
                query: Test::request_get_stats(),
                expected: vec![],
                validity: TestKind::Valid,
            });
        }
    }
    res
}

// Note:
// The following should result in the server from dropping this client
// as conncurrent requests of this kind could lead to DOS due to overuse
// of server resources
fn flood_server() -> Vec<Test> {
    let msg = [97u8; ((((message::MAX_PAYLOAD) * 2) as usize) + 20)];
    vec![test_compress_fail(&msg, Response::MessageTooLarge)]
}
