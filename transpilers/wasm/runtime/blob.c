#include "runtime.h"
#define ZKVM_U256_LE_INLINE
#include "zkvm_u256_le.h"

typedef zkvm_u256_le Fr;
static const Fr modulus = {{UINT64_C(0xffffffff00000001), UINT64_C(0x53bda402fffe5bfe),
                            UINT64_C(0x3339d80809a1d805), UINT64_C(0x73eda753299d7d48)}};
static const Fr one = {{1, 0, 0, 0}};
static const Fr inverse_exponent = {{UINT64_C(0xfffffffeffffffff), UINT64_C(0x53bda402fffe5bfe),
                                     UINT64_C(0x3339d80809a1d805), UINT64_C(0x73eda753299d7d48)}};
/* 7^((r-1)/4096) and 4096^-1 in the BLS scalar field. */
static const Fr root = {{UINT64_C(0xe206da11a5d36306), UINT64_C(0x0ad1347b378fbf96),
                         UINT64_C(0xfc3e8acfe0f8245f), UINT64_C(0x564c0a11a0f704f4)}};
static const Fr inverse_count = {{UINT64_C(0x400fffff00100001), UINT64_C(0xd3686828bfce5c19),
                                  UINT64_C(0x5eb6a46a89213de7), UINT64_C(0x73e66878b46ae370)}};
static Fr roots[4096], prefixes[4096];
static uint8_t roots_ready;
static uint8_t transcript[32 + NITRO_BLOB_BYTES + 48];

static Fr from_bytes(const uint8_t bytes[32]) {
    Fr value = {{0}};
    for (unsigned i = 0; i < 32; ++i)
        value.limbs[(31 - i) / 8] |= (uint64_t)bytes[i] << (8 * ((31 - i) % 8));
    return value;
}
static void to_bytes(const Fr *value, uint8_t bytes[32]) {
    for (unsigned i = 0; i < 32; ++i)
        bytes[i] = value->limbs[(31 - i) / 8] >> (8 * ((31 - i) % 8));
}
static Fr add(Fr a, Fr b) {
    Fr out;
    zkvm_u256_le_addmod(&a, &b, &modulus, &out);
    return out;
}
static Fr sub(Fr a, Fr b) {
    Fr negative;
    zkvm_u256_le_sub(&modulus, &b, &negative);
    return add(a, negative);
}
static Fr mul(Fr a, Fr b) {
    Fr out;
    zkvm_u256_le_mulmod(&a, &b, &modulus, &out);
    return out;
}
static Fr power(Fr base, Fr exponent) {
    Fr out = one;
    for (int bit = 255; bit >= 0; --bit) {
        out = mul(out, out);
        if ((exponent.limbs[bit / 64] >> (bit % 64)) & 1)
            out = mul(out, base);
    }
    return out;
}
static void initialize_roots(void) {
    if (roots_ready)
        return;
    Fr current = one;
    for (unsigned i = 0; i < 4096; ++i) {
        unsigned reversed = 0;
        for (unsigned bit = 0; bit < 12; ++bit)
            reversed = (reversed << 1) | ((i >> bit) & 1);
        roots[reversed] = current;
        current = mul(current, root);
    }
    roots_ready = 1;
}

/* Deneb evaluate_polynomial_in_evaluation_form, with one batched inversion. */
static Fr evaluate(const uint8_t blob[NITRO_BLOB_BYTES], Fr z) {
    initialize_roots();
    int domain_index = -1;
    for (unsigned i = 0; i < 4096; ++i) {
        Fr value = from_bytes(blob + 32 * i);
        if (!zkvm_u256_le_i_ult(value.limbs, modulus.limbs))
            runtime_trap();
        if (!memcmp(&z, &roots[i], sizeof(Fr)))
            domain_index = (int)i;
    }
    /* Validate every scalar even when the challenge is a domain point. */
    if (domain_index >= 0)
        return from_bytes(blob + 32 * domain_index);
    Fr product = one;
    for (unsigned i = 0; i < 4096; ++i) {
        prefixes[i] = product;
        product = mul(product, sub(z, roots[i]));
    }
    Fr inverse = power(product, inverse_exponent);
    Fr sum = {{0}};
    for (int i = 4095; i >= 0; --i) {
        Fr reciprocal = mul(inverse, prefixes[i]);
        inverse = mul(inverse, sub(z, roots[i]));
        Fr value = from_bytes(blob + 32 * i);
        sum = add(sum, mul(mul(value, roots[i]), reciprocal));
    }
    Fr z_to_count = z;
    for (unsigned i = 0; i < 12; ++i)
        z_to_count = mul(z_to_count, z_to_count);
    return mul(sum, mul(sub(z_to_count, one), inverse_count));
}

void runtime_verify_blob(const uint8_t *bytes, uint32_t len, const uint8_t key[32]) {
    /* Type 2 witnesses carry blob || commitment || proof; only the blob is
     * readable. */
    if (len != NITRO_BLOB_BYTES + 96 || key[0] != 1)
        runtime_trap();
    zkvm_kzg_commitment commitment;
    zkvm_kzg_proof proof;
    memcpy(commitment.data, bytes + NITRO_BLOB_BYTES, 48);
    memcpy(proof.data, bytes + NITRO_BLOB_BYTES + 48, 48);
    uint8_t hash[32];
    runtime_sha256(commitment.data, 48, hash);
    hash[0] = 1;
    if (memcmp(hash, key, 32))
        runtime_trap();
    /* Ethereum's Fiat-Shamir transcript binds all blob bytes, including unread
     * ones. */
    memcpy(transcript, "FSBLOBVERIFY_V1_", 16);
    memset(transcript + 16, 0, 16);
    transcript[30] = 0x10; // 4096, encoded as a 16-byte big-endian integer.
    memcpy(transcript + 32, bytes, NITRO_BLOB_BYTES);
    memcpy(transcript + 32 + NITRO_BLOB_BYTES, commitment.data, 48);
    runtime_sha256(transcript, sizeof(transcript), hash);
    Fr z = mul(from_bytes(hash), one);
    Fr y = evaluate(bytes, z);
    zkvm_kzg_field_element z_bytes, y_bytes;
    to_bytes(&z, z_bytes.data);
    to_bytes(&y, y_bytes.data);
    bool verified = false;
    if (zkvm_kzg_point_eval(&commitment, &z_bytes, &y_bytes, &proof, &verified) != ZKVM_EOK ||
        !verified)
        runtime_trap();
}
