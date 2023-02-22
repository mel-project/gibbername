use futures_util::StreamExt;
use themelio_structs::{BlockHeight, CoinData, CoinValue, Denom, Transaction, TxHash};

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
    let snapshot = client.older_snapshot(height).await?;
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
    todo!()
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
