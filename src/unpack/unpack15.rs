use std::io;

pub fn unpack15<R: io::Read + io::Seek>(reader: R) {
    let mut old_dist = [!0, !0, !0, !0];
    let mut old_dist_ptr = 0;

    let mut last_dist = -1;
    let mut last_length = 0;

    let mut unp_ptr = 0;
    let mut wr_ptr = 0;

    let mut prev_ptr = 0;
    let mut first_win_done = false;
}
