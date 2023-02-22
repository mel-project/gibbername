use themelio_structs::{BlockHeight, CoinData};

/// Decodes a gibbername into a blockchain location.
fn decode_gibbername(gname: &str) -> anyhow::Result<(BlockHeight, u32)> {
    let (height, index) = gibbercode::decode(gname);
    Ok((BlockHeight(height as u64), index as u32))
}

/// Encodes a blockchain location into a gibbername.
fn encode_gibbername(height: BlockHeight, index: u32) -> String {
    // get the u64 out of the BlockHeight
    gibbercode::encode(height.0 as u128, index as u128)
}

/// Encodes the given height and index into a gibbername.
pub fn encode_gibbername(height: BlockHeight, index: u32) -> String {
    gibbercode::encode(
        u128::try_from(height.0).unwrap(),
        u128::try_from(index).unwrap(),
    )
}

/// Decodes a given gibbername into the corresponding transaction's height and index.
pub fn decode_gibbername(gibbername: &str) -> anyhow::Result<(BlockHeight, u32)> {
    let (height, index) = gibbercode::decode(gibbername);
    Ok((BlockHeight(height as u64), index as u32))
}

/// Gets and validates the starting transaction of the Gibbername chain.
/// Validation involves checking the transaction for the following properties:
/// 1. The `data` field says "gibbername-v1"
/// 2. The transaction has a single output with the [themelio_structs::Denom::NewCoin] denomination
///    with a value of 1
async fn get_and_validate_start_tx(
    client: &melprot::Client,
    gibbername: &str,
) -> anyhow::Result<(BlockHeight, TxHash)> {
    let (height, index) =
        decode_gibbername(gibbername).expect("failed to decode gibbername: {gibbername}");
    let snapshot = client.older_snapshot(height).await?;
    let txhash = snapshot.get_transaction_by_posn(index as usize).await?;

    // validate the transaction now
    if let Some(txhash) = txhash {
        let tx = snapshot.get_transaction(txhash).await?.unwrap();
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
        // TODO: handle invalid gibbername
        panic!()
    }
}

/// Traverses the Catena chain to get the coin containing the final binding.
async fn traverse_catena_chain(
    client: &melprot::Client,
    start_height: BlockHeight,
    start_txhash: TxHash,
) -> anyhow::Result<CoinData> {
    let traversal: Vec<Transaction> = client
        .traverse_fwd(start_height, start_txhash, |tx: &Transaction| {
            tx.outputs.iter().position(|coin_data| {
                (tx.hash_nosigs() == start_txhash && coin_data.denom == Denom::NewCoin)
                    || coin_data.denom == Denom::Custom(start_txhash)
            })
        })
        .expect("failed to traverse forward")
        .collect()
        .await;
    let last_tx = traversal.last().expect("the traversal is empty");
    Ok(CoinData {})
}

/// Returns the data bound to the given gibbername if there is any.
pub async fn lookup(client: &melprot::Client, gibbername: &str) -> anyhow::Result<String> {
    todo!()
}

#[test]
fn encodes_gibbername() {
    let height = BlockHeight(216);
    let index = 2;
    let gname = encode_gibbername(height, index);

    assert_eq!(gname, "lol".to_string());
}

#[test]
fn decodes_gibbername() {
    let gname = "hehe-lol";
    let (height, index) = gibbercode::decode(gname);

    assert_eq!(height, 216);
    assert_eq!(index, 2);
}
