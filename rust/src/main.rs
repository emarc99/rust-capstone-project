#![allow(unused)]
use bitcoin::hex::DisplayHex;
use bitcoincore_rpc::bitcoin::Amount;
use bitcoincore_rpc::{Auth, Client, RpcApi};
use serde::Deserialize;
use serde_json::json;
use std::fs::File;
use std::io::Write;

// Node access params
const RPC_URL: &str = "http://127.0.0.1:18443"; // Default regtest RPC port
const RPC_USER: &str = "alice";
const RPC_PASS: &str = "password";

// You can use calls not provided in RPC lib API using the generic `call` function.
// An example of using the `send` RPC call, which doesn't have exposed API.
// You can also use serde_json `Deserialize` derivation to capture the returned json result.
fn send(rpc: &Client, addr: &str) -> bitcoincore_rpc::Result<String> {
    let args = [
        json!([{addr : 100 }]), // recipient address
        json!(null),            // conf target
        json!(null),            // estimate mode
        json!(null),            // fee rate in sats/vb
        json!(null),            // Empty option object
    ];

    #[derive(Deserialize)]
    struct SendResult {
        complete: bool,
        txid: String,
    }
    let send_result = rpc.call::<SendResult>("send", &args)?;
    assert!(send_result.complete);
    Ok(send_result.txid)
}

fn main() -> bitcoincore_rpc::Result<()> {
    // Connect to Bitcoin Core RPC
    let rpc = Client::new(
        RPC_URL,
        Auth::UserPass(RPC_USER.to_owned(), RPC_PASS.to_owned()),
    )?;

    // Get blockchain info
    let blockchain_info = rpc.get_blockchain_info()?;
    println!("Blockchain Info: {:?}", blockchain_info);

    // Create/Load the wallets, named 'Miner' and 'Trader'. Have logic to optionally create/load them if they do not exist or not loaded already.
    
    // Try to create Miner wallet, if it exists, load it
    match rpc.create_wallet("Miner", None, None, None, None) {
        Ok(_) => println!("Created Miner wallet"),
        Err(_) => {
            // Wallet might already exist, try to load it
            match rpc.load_wallet("Miner") {
                Ok(_) => println!("Loaded existing Miner wallet"),
                Err(e) => println!("Error with Miner wallet: {}", e),
            }
        }
    }

    // Try to create Trader wallet, if it exists, load it
    match rpc.create_wallet("Trader", None, None, None, None) {
        Ok(_) => println!("Created Trader wallet"),
        Err(_) => {
            // Wallet might already exist, try to load it
            match rpc.load_wallet("Trader") {
                Ok(_) => println!("Loaded existing Trader wallet"),
                Err(e) => println!("Error with Trader wallet: {}", e),
            }
        }
    }

    // Connect to Miner wallet specifically
    let miner_rpc = Client::new(
        "http://127.0.0.1:18443/wallet/Miner",
        Auth::UserPass(RPC_USER.to_owned(), RPC_PASS.to_owned()),
    )?;

    // Connect to Trader wallet specifically
    let trader_rpc = Client::new(
        "http://127.0.0.1:18443/wallet/Trader",
        Auth::UserPass(RPC_USER.to_owned(), RPC_PASS.to_owned()),
    )?;

    // Generate spendable balances in the Miner wallet. How many blocks needs to be mined?
    
    // Generate a new address from the Miner wallet with label "Mining Reward"
    let mining_address = miner_rpc.get_new_address(Some("Mining Reward"), None)?;
    let mining_address = mining_address.require_network(bitcoincore_rpc::bitcoin::Network::Regtest).unwrap();
    println!("Mining address: {}", mining_address);

    // Mine blocks to this address until we get positive wallet balance
    // In regtest, coinbase maturity is 100 blocks, so we need to mine at least 101 blocks
    let mut blocks_mined = 0;
    let mut balance = Amount::ZERO;
    
    while balance == Amount::ZERO {
        // Mine 1 block to the mining address
        let block_hashes = miner_rpc.generate_to_address(1, &mining_address)?;
        blocks_mined += 1;
        println!("Mined block {} to address {}", blocks_mined, mining_address);
        
        // Check balance after mining
        balance = miner_rpc.get_balance(None, None)?;
        println!("Current balance after {} blocks: {} BTC", blocks_mined, balance.to_btc());
        
        // Safety check to avoid infinite loop
        if blocks_mined > 150 {
            break;
        }
    }

    /*
     * Comment about wallet balance behavior:
     * In Bitcoin regtest mode, newly mined coinbase transactions require 100 confirmations 
     * before they become spendable. This is called "coinbase maturity". Therefore, we need 
     * to mine at least 101 blocks before the first coinbase reward becomes available for spending.
     * The wallet balance remains zero until the coinbase outputs mature.
     */

    // Print the balance of the Miner wallet
    let final_balance = miner_rpc.get_balance(None, None)?;
    println!("Final Miner wallet balance: {} BTC", final_balance.to_btc());

    // Load Trader wallet and generate a new address
    
    // Create a receiving address labeled "Received" from Trader wallet
    let trader_address = trader_rpc.get_new_address(Some("Received"), None)?;
    let trader_address = trader_address.require_network(bitcoincore_rpc::bitcoin::Network::Regtest).unwrap();
    println!("Trader receiving address: {}", trader_address);

    // Send 20 BTC from Miner to Trader
    
    let send_amount = Amount::from_btc(20.0).unwrap();
    let txid = miner_rpc.send_to_address(&trader_address, send_amount, None, None, None, None, None, None)?;
    println!("Sent 20 BTC to Trader. Transaction ID: {}", txid);

    // Check transaction in mempool
    
    // Fetch the unconfirmed transaction from the node's mempool
    #[derive(Deserialize)]
    struct MempoolEntry {
        size: u64,
        fee: f64,
        #[serde(rename = "modifiedfee")]
        modified_fee: f64,
        time: u64,
        height: u64,
        #[serde(rename = "descendantcount")]
        descendant_count: u64,
        #[serde(rename = "descendantsize")]
        descendant_size: u64,
        #[serde(rename = "descendantfees")]
        descendant_fees: f64,
        #[serde(rename = "ancestorcount")]
        ancestor_count: u64,
        #[serde(rename = "ancestorsize")]
        ancestor_size: u64,
        #[serde(rename = "ancestorfees")]
        ancestor_fees: f64,
        wtxid: String,
        fees: serde_json::Value,
        depends: Vec<String>,
        #[serde(rename = "spentby")]
        spent_by: Vec<String>,
        #[serde(rename = "bip125-replaceable")]
        bip125_replaceable: bool,
    }

    let mempool_entry: MempoolEntry = rpc.call("getmempoolentry", &[json!(txid.to_string())])?;
    println!("Transaction in mempool: size={}, fee={}", mempool_entry.size, mempool_entry.fee);

    // Mine 1 block to confirm the transaction
    
    let confirm_blocks = miner_rpc.generate_to_address(1, &mining_address)?;
    println!("Mined 1 block to confirm transaction: {:?}", confirm_blocks);

    // Extract all required transaction details
    
    // Get the transaction details from the Miner wallet
    let tx_info = miner_rpc.get_transaction(&txid, Some(true))?;
    println!("Transaction confirmed in block: {}", tx_info.info.blockhash.unwrap());

    // Get the raw transaction to extract detailed input/output information
    let raw_tx = rpc.get_raw_transaction_info(&txid, Some(&tx_info.info.blockhash.unwrap()))?;
    
    // Extract transaction details
    let transaction_id = txid.to_string();
    
    // Get input details (from the first input)
    let input = &raw_tx.vin[0];
    let input_txid = input.txid.unwrap();
    let input_vout = input.vout.unwrap();
    
    // Get the previous transaction to find the input address and amount
    let prev_tx = rpc.get_raw_transaction_info(&input_txid, None)?;
    let prev_output = &prev_tx.vout[input_vout as usize];
    let miner_input_address = prev_output.script_pub_key.address.as_ref().unwrap().clone().assume_checked().to_string();
    let miner_input_amount = prev_output.value.to_btc();

    // Find the outputs
    let mut trader_output_address = String::new();
    let mut trader_output_amount = 0.0;
    let mut miner_change_address = String::new();
    let mut miner_change_amount = 0.0;

    for output in &raw_tx.vout {
        if let Some(address) = &output.script_pub_key.address {
            let address_str = address.clone().assume_checked().to_string();
            let amount = output.value.to_btc();
            
            // Check if this output goes to the trader address
            if address_str == trader_address.to_string() {
                trader_output_address = address_str;
                trader_output_amount = amount;
            } else {
                // This must be the change output back to miner
                miner_change_address = address_str;
                miner_change_amount = amount;
            }
        }
    }

    // Calculate transaction fee
    let transaction_fee = tx_info.fee.unwrap().to_btc().abs();

    // Get block height and hash
    let block_height = tx_info.info.blockheight.unwrap();
    let block_hash = tx_info.info.blockhash.unwrap().to_string();

    // Write the data to ../out.txt in the specified format given in readme.md
    
    let output_data = format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
        transaction_id,
        miner_input_address,
        miner_input_amount,
        trader_output_address,
        trader_output_amount,
        miner_change_address,
        miner_change_amount,
        transaction_fee,
        block_height,
        block_hash
    );

    // Write to out.txt in the parent directory
    let mut file = File::create("../out.txt")?;
    file.write_all(output_data.as_bytes())?;
    
    println!("Transaction details written to out.txt");
    println!("Transaction ID: {}", transaction_id);
    println!("Miner Input Address: {}", miner_input_address);
    println!("Miner Input Amount: {} BTC", miner_input_amount);
    println!("Trader Output Address: {}", trader_output_address);
    println!("Trader Output Amount: {} BTC", trader_output_amount);
    println!("Miner Change Address: {}", miner_change_address);
    println!("Miner Change Amount: {} BTC", miner_change_amount);
    println!("Transaction Fee: {} BTC", transaction_fee);
    println!("Block Height: {}", block_height);
    println!("Block Hash: {}", block_hash);

    Ok(())
}
