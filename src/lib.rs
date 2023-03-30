use anyhow::Context;
use futures_util::StreamExt;
use melstructs::{Address, BlockHeight, CoinData, CoinValue, Denom, Transaction, TxHash};

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
            .filter(|output| output.denom == Denom::NewCustom)
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
            log::debug!("traversing {:?}", tx);
            tx.outputs.iter().position(|coin_data| {
                (tx.hash_nosigs() == start_txhash && coin_data.denom == Denom::NewCustom)
                    || coin_data.denom == Denom::Custom(start_txhash)
            })
        })
        .expect("failed to traverse forward")
        .collect::<Vec<Transaction>>()
        .await;

    if traversal.is_empty() {
        let snap = client.snapshot(start_height).await?;
        let tx = snap
            .get_transaction(start_txhash)
            .await?
            .context("No transaction with given hash")?;
        let coin = tx
            .outputs
            .iter()
            .find(|coin| coin.denom == Denom::NewCustom);

        match coin {
            Some(coin_data) => return Ok(coin_data.clone()),
            None => anyhow::bail!("No valid gibbercoins found"),
        }
    }

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

/// Traverses the Catena chain to get the coin containing all the historical bindings.
async fn traverse_catena_chain_whole_history(
    client: &melprot::Client,
    start_height: BlockHeight,
    start_txhash: TxHash,
) -> anyhow::Result<Vec<CoinData>> {
    let traversal = client
        .traverse_fwd(start_height, start_txhash, move |tx: &Transaction| {
            log::debug!("traversing {:?}", tx);
            tx.outputs.iter().position(|coin_data| {
                (tx.hash_nosigs() == start_txhash && coin_data.denom == Denom::NewCustom)
                    || coin_data.denom == Denom::Custom(start_txhash)
            })
        })
        .expect("failed to traverse forward")
        .collect::<Vec<Transaction>>()
        .await;

    println!("{:?}", traversal);

    let mut ret = vec![];

    let snap = client.snapshot(start_height).await?;
    let tx = snap
        .get_transaction(start_txhash)
        .await?
        .context("No transaction with given hash")?;
    let coin = tx
        .outputs
        .iter()
        .find(|coin| coin.denom == Denom::NewCustom);

    match coin {
        Some(coin_data) => ret.push(coin_data.clone()),
        None => anyhow::bail!("No valid gibbercoins found"),
    }

    let lala: anyhow::Result<Vec<CoinData>> = traversal
        .iter()
        .map(|tx| {
            if let Some(coin_data) = tx.outputs.iter().find(|coin_data| {
                coin_data.denom == Denom::Custom(start_txhash)
                    || coin_data.denom == Denom::NewCustom
            }) {
                Ok(coin_data.clone())
            } else {
                anyhow::bail!("OH NO! catena chain BROKE in the middle!")
            }
        })
        .collect();
    ret.extend(lala?);

    Ok(ret)
}

/// Returns the data bound to the given gibbername if there is any.
pub async fn lookup(client: &melprot::Client, gibbername: &str) -> anyhow::Result<String> {
    let (start_height, start_txhash) = get_and_validate_start_tx(client, gibbername).await?;
    log::debug!("start_height: {start_height}, start_txhash: {start_txhash}");
    let last_coin = traverse_catena_chain(client, start_height, start_txhash).await?;
    let binding = String::from_utf8_lossy(&last_coin.additional_data);

    Ok(binding.into_owned())
}

/// Returns all the data ever bound to the given gibbername, if there is any
pub async fn lookup_whole_history(
    client: &melprot::Client,
    gibbername: &str,
) -> anyhow::Result<Vec<String>> {
    let (start_height, start_txhash) = get_and_validate_start_tx(client, gibbername).await?;
    log::debug!("start_height: {start_height}, start_txhash: {start_txhash}");
    let all_coins = traverse_catena_chain_whole_history(client, start_height, start_txhash).await?;
    let bindings: Vec<String> = all_coins
        .iter()
        .map(|coin| String::from_utf8_lossy(&coin.additional_data).into_owned())
        .collect();
    Ok(bindings)
}

#[allow(unused)]
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
    todo!()
}

fn register_name_cmd(
    wallet_name: &str,
    address: Address,
    initial_binding: &str,
) -> anyhow::Result<String> {
    let cmd = format!(
        "melwallet-cli send -w {} --to {},{},{},\"{}\" --hex-data {}",
        wallet_name,
        address,
        0.000001,
        "\"(NEWCUSTOM)\"",
        hex::encode(initial_binding),
        hex::encode("gibbername-v1")
    );

    Ok(cmd)
}

pub async fn register(
    client: &melprot::Client,
    address: Address,
    initial_binding: &str,
    wallet_name: &str,
) -> anyhow::Result<String> {
    let height = client.latest_snapshot().await?.current_header().height;
    let cmd = register_name_cmd(wallet_name, address, initial_binding)?;
    println!("Send this command with your wallet: {}", cmd);

    // scan through all transactions involving this address, starting at the block height right before we asked the user to send the transacton
    let mut stream = client.stream_transactions_from(height, address).boxed();
    while let Some((transaction, height)) = stream.next().await {
        if &transaction.data[..] == b"gibbername-v1" {
            let txhash = transaction.hash_nosigs();
            let (posn, _) = client
                .snapshot(height)
                .await?
                .current_block()
                .await?
                .abbreviate()
                .txhashes
                .iter()
                .enumerate()
                .find(|(_, hash)| **hash == txhash)
                .expect("No transaction with matching hash in this block.");

            let gibbername = encode_gibbername(height, posn as u32)?;
            return Ok(gibbername);
        }
    }
    unreachable!()
}

pub async fn transfer_name_cmd(
    client: &melprot::Client,
    gibbername: &str,
    wallet_name: &str,
    address: Address,
    new_binding: &str,
) -> anyhow::Result<()> {
    let current_height = client.latest_snapshot().await?.current_header().height;
    let (height, index) = decode_gibbername(gibbername)?;

    let snap = client.snapshot(height).await?;
    let txhash = snap
        .get_transaction_by_posn(index as usize)
        .await?
        .context("Couldn't find tx for given gibbername")?;
    let denom = Denom::Custom(txhash);

    let cmd = format!(
        "melwallet-cli send -w {} --to {},{},{},{}",
        wallet_name,
        address,
        0.000001,
        denom,
        hex::encode(new_binding),
    );

    println!("Send this command with your wallet: {}", cmd);

    // scan through all transactions involving this address, starting at the block height right before we asked the user to send the transacton
    let mut stream = client
        .stream_transactions_from(current_height, address)
        .boxed();
    while let Some((transaction, _height)) = stream.next().await {
        if let Some(coin) = &transaction
            .outputs
            .iter()
            .find(|coin| String::from_utf8_lossy(&coin.additional_data) == new_binding)
        {
            println!("COIN_DATA: {:?}", coin);
            println!(
                "Gibbername {} transferred to {} with new binding {}",
                gibbername, address, new_binding
            );
            return Ok(());
        }
    }
    unreachable!()
}

#[cfg(test)]
mod test {
    use super::*;
    use melstructs::NetID;
    use std::str::FromStr;

    #[test]
    fn end2end() -> anyhow::Result<()> {
        let _ = env_logger::try_init();
        smolscale::block_on(async {
            let client = melprot::Client::autoconnect(NetID::Mainnet).await.unwrap();
            let address =
                Address::from_str("t2k917e3f3r6wk5474sg3exmfpkh04a42w1chmek68fv5pnygywvsg")
                    .unwrap();
            let initial_binding = "henlo world lmao";
            let wallet_name = "alice";

            let gibbername = register(&client, address, initial_binding, wallet_name)
                .await
                .unwrap();

            println!("gibbername: {gibbername}");
            let binding = lookup(&client, &gibbername).await.unwrap();
            println!("INITIAL BINDING: {}", binding);

            let new_binding = "it is wednesday my dudes";
            transfer_name_cmd(&client, &gibbername, wallet_name, address, new_binding)
                .await
                .unwrap();

            let new_new_binding = "it's actually thursday my dudes";
            transfer_name_cmd(&client, &gibbername, wallet_name, address, new_new_binding)
                .await
                .unwrap();

            let final_lookup = lookup(&client, &gibbername).await.unwrap();
            println!("FINAL LOOKUP: {}", final_lookup);

            let whole_history = lookup_whole_history(&client, &gibbername).await.unwrap();
            println!("WHOLE HISTORY: {:?}", whole_history);
        });

        Ok(())
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

    #[test]
    fn make_new_gibbername() {}
}
