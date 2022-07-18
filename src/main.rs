use clap::Parser;
use ethers_core::types::{Address, BlockNumber, Filter, U64};
// use ethers_etherscan::Client;
use ethers_providers::{Http, Middleware, Provider};
use eyre::Result;
use futures::{stream, StreamExt};
use serde_json::json;
use std::cmp;
use std::time::Instant;

const BATCH_SIZE: i64 = 9999; // eth logs infura only allows 10K blocks
const PARALLEL_REQUESTS: usize = 10;

#[derive(Parser, Debug)]
#[clap(about, version, author)]
struct Args {
    /// contract address to fetch logs
    #[clap(short = 'a', long)]
    address: String,
    /// ethereum node RPC URL
    #[clap(short = 'u', long)]
    eth_url: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let address = args.address;
    let eth_url = args.eth_url;

    let vault_addr = address.parse::<Address>().unwrap();

    // let etherscan_client = Client::new(Chain::Mainnet, ETHERSCAN_KEY).unwrap();
    // let _abi = etherscan_client.contract_abi(vault_addr).await.unwrap();

    let client = Provider::<Http>::try_from(eth_url).expect("error connecting to provider");

    let last_block = client
        .get_block(BlockNumber::Latest)
        .await?
        .unwrap()
        .number
        .unwrap();
    println!("last_block: {}", last_block);

    let page_vector = build_page_vector(last_block);
    let ref_client = &client;

    let tasks = stream::iter(page_vector)
        .map(|(start, end)| async move { fetch_logs(ref_client, vault_addr, start, end).await })
        .buffered(PARALLEL_REQUESTS);

    tasks
        .for_each(|r| async {
            match r {
                Ok(r) => println!("{:?}", r),
                Err(e) => eprintln!("Got an error: {}", e),
            }
        })
        .await;

    Ok(())
}

fn build_page_vector(last_block: U64) -> Vec<(i64, i64)> {
    let mut page_vector: Vec<(i64, i64)> = vec![];
    let mut n: i64 = 0;
    let last_block_i64 = last_block.as_u64() as i64;
    while n < (last_block_i64) {
        page_vector.push((n, cmp::min(n + BATCH_SIZE, last_block_i64)));
        n = n + BATCH_SIZE + 1;
    }
    page_vector
}

async fn fetch_logs(
    client: &Provider<Http>,
    vault_addr: ethers_core::types::H160,
    start: i64,
    end: i64,
) -> Result<String> {
    let log_filter = Filter::new()
        .from_block(start)
        .to_block(end)
        .address(vault_addr);

    let start_counter = Instant::now();
    let logs = client.get_logs(&log_filter).await?;

    // println!("{:?}", logs);

    let elapsed = start_counter.elapsed().as_millis();
    let rate = logs.len() as u128 / elapsed; // rate in millis
    let res_json = json!({ "threads": PARALLEL_REQUESTS, "page_size":  BATCH_SIZE, "elapsed": elapsed.to_string(), "rate": rate.to_string()} );
    let res = format!("{}\n", res_json);
    println!("{}", res);
    Ok(res)
}
