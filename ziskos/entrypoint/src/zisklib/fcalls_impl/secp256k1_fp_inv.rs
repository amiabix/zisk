use lib_c::secp256k1_fp_inv_c;

pub fn secp256k1_fp_inv(params: &[u64], results: &mut [u64]) -> i64 {
    // Perform the inversion
    let res_c_call = secp256k1_fp_inv_c(params, results);
    if res_c_call == 0 {
        4
    } else {
        res_c_call as i64
    }
}
