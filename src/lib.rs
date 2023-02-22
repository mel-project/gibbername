use themelio_structs::BlockHeight;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
