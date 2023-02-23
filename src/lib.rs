use std::str::FromStr;

use futures_util::StreamExt;
use themelio_structs::{
    Address, BlockHeight, CoinData, CoinValue, Denom, Transaction, TxHash, TxKind,
};
use tmelcrypt::{Ed25519SK, HashVal};

/// Decodes a gibbername into a blockchain location.
fn decode_gibbername(gname: &str) -> anyhow::Result<(BlockHeight, u32)> {
    let (height, index) = gibbercode::decode(gname);
    Ok((BlockHeight(height as u64), index as u32))
}

/// Encodes the given height and index into a gibbername.
fn encode_gibbername(height: BlockHeight, index: u32) -> anyhow::Result<String> {
    Ok(gibbercode::encode(
        u128::try_from(height.0)?,
        u128::try_from(index)?,
    ))
}

/// Gets and validates the starting transaction of the gibbername chain.
/// Validation involves checking the transaction for the following properties:
/// 1. The `data` field says "gibbername-v1"
/// 2. The transaction has a single output with the [themelio_structs::Denom::NewCoin] denomination
///    with a value of 1
async fn get_and_validate_start_tx(
    client: &melprot::Client,
    gibbername: &str,
) -> anyhow::Result<(BlockHeight, TxHash)> {
    let (height, index) = decode_gibbername(gibbername).expect("failed to decode {gibbername}");
    let snapshot = client.snapshot(height).await?;
    let txhash = snapshot.get_transaction_by_posn(index as usize).await?;

    // validate the transaction now
    if let Some(txhash) = txhash {
        let tx = snapshot
            .get_transaction(txhash)
            .await?
            .expect("expected transaction to exist, because txhash exists");

        // check the data
        if &tx.data[..] != b"gibbername-v1" {
            anyhow::bail!("invalid data in the start transaction: {:?}", tx.data);
        }

        let new_outputs = tx
            .outputs
            .iter()
            .filter(|output| output.denom == Denom::NewCoin)
            .collect::<Vec<&CoinData>>();
        if new_outputs.len() == 1 && new_outputs[0].value == CoinValue(1) {
            Ok((height, tx.hash_nosigs()))
        } else {
            anyhow::bail!("invalid start transaction outputs");
        }
    } else {
        anyhow::bail!("could not find starting transaction for the given gibbername: {gibbername}");
    }
}

/// Traverses the Catena chain to get the coin containing the final binding.
async fn traverse_catena_chain(
    client: &melprot::Client,
    start_height: BlockHeight,
    start_txhash: TxHash,
) -> anyhow::Result<CoinData> {
    let traversal = client
        .traverse_fwd(start_height, start_txhash, move |tx: &Transaction| {
            tx.outputs.iter().position(|coin_data| {
                (tx.hash_nosigs() == start_txhash && coin_data.denom == Denom::NewCoin)
                    || coin_data.denom == Denom::Custom(start_txhash)
            })
        })
        .expect("failed to traverse forward")
        .collect::<Vec<Transaction>>()
        .await;

    let last_tx = traversal.last().expect("the traversal is empty");
    if let Some(last_tx_coin) = last_tx
        .outputs
        .iter()
        .find(|coin_data| coin_data.denom == Denom::Custom(start_txhash))
    {
        Ok(last_tx_coin.clone())
    } else {
        anyhow::bail!("the name was permanently deleted");
    }
}

/// Returns the data bound to the given gibbername if there is any.
pub async fn lookup(client: &melprot::Client, gibbername: &str) -> anyhow::Result<String> {
    let (start_height, start_txhash) = get_and_validate_start_tx(client, gibbername).await?;
    let last_coin = traverse_catena_chain(client, start_height, start_txhash).await?;
    let binding = String::from_utf8_lossy(&last_coin.additional_data);
    Ok(binding.into_owned())
}

fn register_name_uri(address: Address, initial_binding: &str) -> String {
    // melwallet_uri::MwUriBuilder::new()
    //     .output(0, CoinData {
    //         denom: NewCoin::Denom,
    //         value: 1.into(),
    //         covhash: address,
    //         additional_data: initial_binding.as_bytes().into(),
    //     })
    //     .data(b"gibbername-v1")
    //     .build()
    String::new()
}

fn register_name_tx(address: Address, initial_binding: String) -> anyhow::Result<Transaction> {
    let output = CoinData {
        covhash: address,
        value: CoinValue(1),
        denom: Denom::NewCoin,
        additional_data: initial_binding.into(),
    };

    let tx = Transaction {
        kind: TxKind::Normal,
        inputs: vec![],
        outputs: vec![output],
        fee: CoinValue(0),
        covenants: vec![],
        data: "gibbername-v1".into(),
        sigs: vec![],
    };

    Ok(tx)
}

pub async fn register(
    client: &melprot::Client,
    address: Address,
    initial_binding: &str,
) -> anyhow::Result<String> {
    let current_height = client.latest_snapshot().await?.current_header().height;
    let uri = register_name_uri(address, initial_binding);
    println!("send with your wallet: {}", uri);

    // scan through all transactions involving this address, starting at the block height right before we asked the user to send the transacton
    // we use a Stream-based API
    let stream = client.stream_transactions(current_height, address).boxed();
    while let Some(transaction) = stream.next().await {
        if transaction.data == b"gibbername-v1".into() {
            return Ok(encode_gibbername(height, posn)?);
        }
    }
    unreachable!()
}

#[test]
fn main() {
    let sk = Ed25519SK::generate();
    let address = Address(HashVal(sk.to_public().0));
    let binding = String::from("henlo world");
    let mut tx = register_name_tx(address, binding).unwrap();
    let sig = sk.sign(&tx.hash_nosigs().0);
    tx.sigs = vec![sig.into()];
    let tx_bytes = stdcode::serialize(&tx).unwrap();

    println!(
        "sk: {}, address: {}\ntx: {}",
        hex::encode(sk.0),
        address,
        hex::encode(tx_bytes)
    );
}

// TODO: use something that's not "hehe"
#[test]
fn encodes_gibbername() {
    let height = BlockHeight(216);
    let index = 2;
    let gname = encode_gibbername(height, index);

    assert_eq!(gname.unwrap(), "lol".to_string());
}

#[test]
fn decodes_gibbername() {
    let gname = "hehe-lol";
    let (height, index) = gibbercode::decode(gname);

    assert_eq!(height, 216);
    assert_eq!(index, 2);
}
