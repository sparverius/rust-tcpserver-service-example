/// A simplified prefix encoding compression scheme. Replace all consecutively
/// repeated characters in the given string by a prefix denoting the number of
/// characters replaced followed by the character itself.
///
/// Some examples:
/// a => a
/// aa => aa
/// aaa => 3a
/// aaaaabbb => 5a3b
/// aaaaabbbbbbaaabb => 5a6b3abb
/// abcdefg => abcdefg
/// aaaccddddhhhhi => 3acc4d4hi
///
/// # Example
/// ```
/// # use service::compress_message;
/// let rx = [97u8, 97, 97];
/// let mut tx = [0u8; 3];
/// let answer = compress_message(&rx, &mut tx).unwrap();
/// assert_eq!(tx[..answer], [51, 97]);
/// ```
/// Must be validated already
pub fn compress_message(rx: &[u8], tx: &mut [u8]) -> Option<usize> {
    let len = rx.len();
    let mut count = 1;
    let mut compress = 0;
    if len == 0 || (rx.len() > tx.len()) {
        return None;
    }
    for i in 0..len {
        if i == len - 1 || rx[i] != rx[i + 1] {
            if count == 2 {
                tx[compress] = rx[i];
                compress += 1;
            }
            if count > 2 {
                for c in count.to_string().bytes() {
                    tx[compress] = c;
                    compress += 1;
                }
            }
            tx[compress] = rx[i];
            compress += 1;
            count = 0;
        }
        count += 1
    }
    Some(compress)
}

#[cfg(test)]
mod tests {
    use super::compress_message;

    #[test]
    fn test_none() {
        let result = compress_message(&[], &mut []);
        assert_eq!(result, None);
    }

    #[test]
    fn test_compress_message() {
        fn test_some(rx: &[u8], expect: &[u8]) {
            let mut tx = [0; 32];
            let res = compress_message(&rx, &mut tx);
            assert_eq!(&tx[..res.unwrap()], expect);
        }

        test_some(&[97u8], &[97]);
        test_some(&[97u8, 97], &[97, 97]);
        test_some(&[97u8, 97, 97], &[51, 97]);
        test_some(&[97u8, 97, 97, 98], &[51, 97, 98]);
        test_some(&[97u8, 97, 98, 98], &[97, 97, 98, 98]);

        let msg = [97u8, 97, 97, 97, 97, 97, 97, 97, 97, 97];
        test_some(&msg, &[49, 48, 97]);

        let msg = [97u8, 97, 97, 97, 97, 97, 97, 97, 97, 97, 97];
        test_some(&msg, &[49, 49, 97]);
    }
}
