use rand::Rng;
use rand::distr::Alphanumeric;

use crate::generator::GenChannel;

pub fn gen_chars(gnc: &mut GenChannel, cnt: usize, up_case: bool) -> String {
    let one: String = std::iter::repeat(())
        .map(|()| gnc.rng.sample(Alphanumeric))
        .map(char::from)
        .take(cnt)
        .collect();
    if up_case { one.to_uppercase() } else { one }
}
