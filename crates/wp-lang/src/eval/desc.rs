pub fn group_idx_desc(idx: usize) -> &'static str {
    match idx {
        0 => "",
        1 => "group[1]",
        2 => "group[2]",
        3 => "group[3]",
        4 => "group[4]",
        5 => "group[5]",
        6 => "group[6]",
        7 => "group[7]",
        8 => "group[8]",
        9 => "group[9]",
        _ => "group[.]",
    }
}

#[allow(dead_code)]
pub fn fpu_idx_desc(idx: usize) -> &'static str {
    match idx {
        0 => "",
        1 => "field[1]",
        2 => "field[2]",
        3 => "field[3]",
        4 => "field[4]",
        5 => "field[5]",
        6 => "field[6]",
        7 => "field[7]",
        8 => "field[8]",
        9 => "field[9]",
        _ => "field[.]",
    }
}
