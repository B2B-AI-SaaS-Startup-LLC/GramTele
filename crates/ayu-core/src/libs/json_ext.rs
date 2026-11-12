//! Byte compatibility with nlohmann/json's Grisu2 double formatting.
//! Algorithm and constants translated from AyuGram's MIT-licensed json.hpp
//! (Niels Lohmann and contributors); no C++ code is linked into the port.
#[derive(Clone, Copy)]
struct Diy {
    f: u64,
    e: i32,
}
impl Diy {
    fn normalize(self) -> Self {
        let shift = self.f.leading_zeros();
        Self {
            f: self.f << shift,
            e: self.e - shift as i32,
        }
    }
    fn mul(self, other: Self) -> Self {
        let product = self.f as u128 * other.f as u128;
        Self {
            f: ((product + (1_u128 << 63)) >> 64) as u64,
            e: self.e + other.e + 64,
        }
    }
}
fn rounded(buf: &mut [u8], dist: u64, delta: u64, mut rest: u64, ten: u64) {
    while rest < dist
        && delta - rest >= ten
        && (rest + ten < dist || dist - rest > rest + ten - dist)
    {
        *buf.last_mut().unwrap() -= 1;
        rest += ten;
    }
}
pub fn format_double(value: f64) -> String {
    if !value.is_finite() {
        return "null".into();
    }
    let sign = if value.is_sign_negative() { "-" } else { "" };
    let value = value.abs();
    if value == 0. {
        return format!("{sign}0.0");
    }
    let bits = value.to_bits();
    let exp = (bits >> 52) as i32;
    let frac = bits & ((1 << 52) - 1);
    let v = Diy {
        f: if exp == 0 { frac } else { frac + (1 << 52) },
        e: if exp == 0 { -1074 } else { exp - 1075 },
    };
    let plus = Diy {
        f: 2 * v.f + 1,
        e: v.e - 1,
    }
    .normalize();
    let minus = if frac == 0 && exp > 1 {
        Diy {
            f: 4 * v.f - 1,
            e: v.e - 2,
        }
    } else {
        Diy {
            f: 2 * v.f - 1,
            e: v.e - 1,
        }
    };
    let minus = Diy {
        f: minus.f << (minus.e - plus.e),
        e: plus.e,
    };
    let v = v.normalize();
    let f = -60 - plus.e - 1;
    let k = f * 78913 / (1 << 18) + i32::from(f > 0);
    let index = (300 + k + 7) / 8;
    let (cf, ce, ck) = CACHED[index as usize];
    let c = Diy { f: cf, e: ce };
    let w = v.mul(c);
    let lo = minus.mul(c);
    let hi = plus.mul(c);
    let high = hi.f - 1;
    let mut delta = high - (lo.f + 1);
    let mut dist = high - w.f;
    let shift = -hi.e;
    let one = 1_u64 << shift;
    let mut p1 = (high >> shift) as u32;
    let mut p2 = high & (one - 1);
    let mut pow10 = 1_u32;
    let mut n = 1;
    while p1 / pow10 >= 10 {
        pow10 *= 10;
        n += 1;
    }
    let mut digits = Vec::new();
    let mut decimal_exp = -ck;
    loop {
        let d = p1 / pow10;
        p1 %= pow10;
        digits.push(b'0' + d as u8);
        n -= 1;
        let rest = ((p1 as u64) << shift) + p2;
        if rest <= delta {
            decimal_exp += n;
            rounded(&mut digits, dist, delta, rest, (pow10 as u64) << shift);
            break;
        }
        pow10 /= 10;
        if n == 0 {
            let mut m = 0;
            loop {
                p2 *= 10;
                digits.push(b'0' + (p2 >> shift) as u8);
                p2 &= one - 1;
                m += 1;
                delta *= 10;
                dist *= 10;
                if p2 <= delta {
                    break;
                }
            }
            decimal_exp -= m;
            rounded(&mut digits, dist, delta, p2, one);
            break;
        }
    }
    let digits = String::from_utf8(digits).unwrap();
    let len = digits.len() as i32;
    let n = len + decimal_exp;
    let body = if len <= n && n <= 15 {
        format!("{}{}.0", digits, "0".repeat((n - len) as usize))
    } else if 0 < n && n <= 15 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -4 < n && n <= 0 {
        format!("0.{}{}", "0".repeat((-n) as usize), digits)
    } else {
        let exponent = n - 1;
        let mantissa = if len == 1 {
            digits
        } else {
            format!("{}.{}", &digits[..1], &digits[1..])
        };
        format!(
            "{mantissa}e{}{:02}",
            if exponent >= 0 { "+" } else { "-" },
            exponent.abs()
        )
    };
    format!("{sign}{body}")
}

const CACHED: [(u64, i32, i32); 79] = [
    (0xAB70FE17C79AC6CA, -1060, -300),
    (0xFF77B1FCBEBCDC4F, -1034, -292),
    (0xBE5691EF416BD60C, -1007, -284),
    (0x8DD01FAD907FFC3C, -980, -276),
    (0xD3515C2831559A83, -954, -268),
    (0x9D71AC8FADA6C9B5, -927, -260),
    (0xEA9C227723EE8BCB, -901, -252),
    (0xAECC49914078536D, -874, -244),
    (0x823C12795DB6CE57, -847, -236),
    (0xC21094364DFB5637, -821, -228),
    (0x9096EA6F3848984F, -794, -220),
    (0xD77485CB25823AC7, -768, -212),
    (0xA086CFCD97BF97F4, -741, -204),
    (0xEF340A98172AACE5, -715, -196),
    (0xB23867FB2A35B28E, -688, -188),
    (0x84C8D4DFD2C63F3B, -661, -180),
    (0xC5DD44271AD3CDBA, -635, -172),
    (0x936B9FCEBB25C996, -608, -164),
    (0xDBAC6C247D62A584, -582, -156),
    (0xA3AB66580D5FDAF6, -555, -148),
    (0xF3E2F893DEC3F126, -529, -140),
    (0xB5B5ADA8AAFF80B8, -502, -132),
    (0x87625F056C7C4A8B, -475, -124),
    (0xC9BCFF6034C13053, -449, -116),
    (0x964E858C91BA2655, -422, -108),
    (0xDFF9772470297EBD, -396, -100),
    (0xA6DFBD9FB8E5B88F, -369, -92),
    (0xF8A95FCF88747D94, -343, -84),
    (0xB94470938FA89BCF, -316, -76),
    (0x8A08F0F8BF0F156B, -289, -68),
    (0xCDB02555653131B6, -263, -60),
    (0x993FE2C6D07B7FAC, -236, -52),
    (0xE45C10C42A2B3B06, -210, -44),
    (0xAA242499697392D3, -183, -36),
    (0xFD87B5F28300CA0E, -157, -28),
    (0xBCE5086492111AEB, -130, -20),
    (0x8CBCCC096F5088CC, -103, -12),
    (0xD1B71758E219652C, -77, -4),
    (0x9C40000000000000, -50, 4),
    (0xE8D4A51000000000, -24, 12),
    (0xAD78EBC5AC620000, 3, 20),
    (0x813F3978F8940984, 30, 28),
    (0xC097CE7BC90715B3, 56, 36),
    (0x8F7E32CE7BEA5C70, 83, 44),
    (0xD5D238A4ABE98068, 109, 52),
    (0x9F4F2726179A2245, 136, 60),
    (0xED63A231D4C4FB27, 162, 68),
    (0xB0DE65388CC8ADA8, 189, 76),
    (0x83C7088E1AAB65DB, 216, 84),
    (0xC45D1DF942711D9A, 242, 92),
    (0x924D692CA61BE758, 269, 100),
    (0xDA01EE641A708DEA, 295, 108),
    (0xA26DA3999AEF774A, 322, 116),
    (0xF209787BB47D6B85, 348, 124),
    (0xB454E4A179DD1877, 375, 132),
    (0x865B86925B9BC5C2, 402, 140),
    (0xC83553C5C8965D3D, 428, 148),
    (0x952AB45CFA97A0B3, 455, 156),
    (0xDE469FBD99A05FE3, 481, 164),
    (0xA59BC234DB398C25, 508, 172),
    (0xF6C69A72A3989F5C, 534, 180),
    (0xB7DCBF5354E9BECE, 561, 188),
    (0x88FCF317F22241E2, 588, 196),
    (0xCC20CE9BD35C78A5, 614, 204),
    (0x98165AF37B2153DF, 641, 212),
    (0xE2A0B5DC971F303A, 667, 220),
    (0xA8D9D1535CE3B396, 694, 228),
    (0xFB9B7CD9A4A7443C, 720, 236),
    (0xBB764C4CA7A44410, 747, 244),
    (0x8BAB8EEFB6409C1A, 774, 252),
    (0xD01FEF10A657842C, 800, 260),
    (0x9B10A4E5E9913129, 827, 268),
    (0xE7109BFBA19C0C9D, 853, 276),
    (0xAC2820D9623BF429, 880, 284),
    (0x80444B5E7AA7CF85, 907, 292),
    (0xBF21E44003ACDD2D, 933, 300),
    (0x8E679C2F5E44FF8F, 960, 308),
    (0xD433179D9C8CB841, 986, 316),
    (0x9E19DB92B4E31BA9, 1013, 324),
];

/// libstdc++ x86_64 iteration order, used by the native Linux reference.
/// C++ itself does not promise a portable order for unordered_set serialization.
pub fn reserve_buckets(size: usize) -> usize {
    PRIMES
        .iter()
        .copied()
        .find(|n| *n >= size.max(2))
        .unwrap_or(*PRIMES.last().unwrap())
}
pub fn set_insert(ids: &mut Vec<i64>, buckets: &mut usize, id: i64) -> bool {
    if ids.contains(&id) {
        return false;
    }
    if ids.len() + 1 > *buckets {
        *buckets =
            reserve_buckets((ids.len() + 1).max(if *buckets == 0 { 13 } else { 2 * *buckets }));
        let previous = std::mem::take(ids);
        for id in previous {
            insert_bucket(ids, *buckets, id);
        }
    }
    insert_bucket(ids, *buckets, id);
    true
}
fn insert_bucket(ids: &mut Vec<i64>, buckets: usize, id: i64) {
    let bucket = id as u64 % buckets as u64;
    let position = ids
        .iter()
        .position(|v| *v as u64 % buckets as u64 == bucket)
        .unwrap_or(0);
    ids.insert(position, id);
}
pub fn set_from_iter(input: impl IntoIterator<Item = i64>) -> (Vec<i64>, usize) {
    let input = input.into_iter().collect::<Vec<_>>();
    let mut ids = Vec::new();
    let mut buckets = reserve_buckets(input.len());
    for id in input {
        set_insert(&mut ids, &mut buckets, id);
    }
    (ids, buckets)
}
const PRIMES: &[usize] = &[
    1,
    2,
    3,
    5,
    7,
    11,
    13,
    17,
    19,
    23,
    29,
    31,
    37,
    41,
    43,
    47,
    53,
    59,
    61,
    67,
    71,
    73,
    79,
    83,
    89,
    97,
    103,
    109,
    113,
    127,
    137,
    139,
    149,
    157,
    167,
    179,
    193,
    199,
    211,
    227,
    241,
    257,
    277,
    293,
    313,
    337,
    359,
    383,
    409,
    439,
    467,
    503,
    541,
    577,
    619,
    661,
    709,
    761,
    823,
    887,
    953,
    1031,
    1109,
    1193,
    1289,
    1381,
    1493,
    1613,
    1741,
    1879,
    2029,
    2179,
    2357,
    2549,
    2753,
    2971,
    3209,
    3469,
    3739,
    4027,
    4349,
    4703,
    5087,
    5503,
    5953,
    6427,
    6949,
    7517,
    8123,
    8783,
    9497,
    10273,
    11113,
    12011,
    12983,
    14033,
    15173,
    16411,
    17749,
    19183,
    20753,
    22447,
    24281,
    26267,
    28411,
    30727,
    33223,
    35933,
    38873,
    42043,
    45481,
    49201,
    53201,
    57557,
    62233,
    67307,
    72817,
    78779,
    85229,
    92203,
    99733,
    107897,
    116731,
    126271,
    136607,
    147793,
    159871,
    172933,
    187091,
    202409,
    218971,
    236897,
    256279,
    277261,
    299951,
    324503,
    351061,
    379787,
    410857,
    444487,
    480881,
    520241,
    562841,
    608903,
    658753,
    712697,
    771049,
    834181,
    902483,
    976369,
    1056323,
    1142821,
    1236397,
    1337629,
    1447153,
    1565659,
    1693859,
    1832561,
    1982627,
    2144977,
    2320627,
    2510653,
    2716249,
    2938679,
    3179303,
    3439651,
    3721303,
    4026031,
    4355707,
    4712381,
    5098259,
    5515729,
    5967347,
    6456007,
    6984629,
    7556579,
    8175383,
    8844859,
    9569143,
    10352717,
    11200489,
    12117689,
    13109983,
    14183539,
    15345007,
    16601593,
    17961079,
    19431899,
    21023161,
    22744717,
    24607243,
    26622317,
    28802401,
    31160981,
    33712729,
    36473443,
    39460231,
    42691603,
    46187573,
    49969847,
    54061849,
    58488943,
    63278561,
    68460391,
    74066549,
    80131819,
    86693767,
    93793069,
    101473717,
    109783337,
    118773397,
    128499677,
    139022417,
    150406843,
    162723577,
    176048909,
    190465427,
    206062531,
    222936881,
    241193053,
    260944219,
    282312799,
    305431229,
    330442829,
    357502601,
    386778277,
    418451333,
    452718089,
    489790921,
    529899637,
    573292817,
    620239453,
    671030513,
    725980837,
    785430967,
    849749479,
    919334987,
    994618837,
    1076067617,
    1164186217,
    1259520799,
    1362662261,
    1474249943,
    1594975441,
    1725587117,
    1866894511,
    2019773507,
    2185171673,
    2364114217,
    2557710269,
    2767159799,
    2993761039,
    3238918481,
    3504151727,
    3791104843,
    4101556399,
    4294967291,
    6442450933,
    8589934583,
    12884901857,
    17179869143,
    25769803693,
    34359738337,
    51539607367,
    68719476731,
    103079215087,
    137438953447,
    206158430123,
    274877906899,
    412316860387,
    549755813881,
    824633720731,
    1099511627689,
    1649267441579,
    2199023255531,
    3298534883309,
    4398046511093,
    6597069766607,
    8796093022151,
    13194139533241,
    17592186044399,
    26388279066581,
    35184372088777,
    52776558133177,
    70368744177643,
    105553116266399,
    140737488355213,
    211106232532861,
    281474976710597,
    562949953421231,
    1125899906842597,
    2251799813685119,
    4503599627370449,
    9007199254740881,
    18014398509481951,
    36028797018963913,
    72057594037927931,
    144115188075855859,
    288230376151711717,
    576460752303423433,
    1152921504606846883,
    2305843009213693951,
    4611686018427387847,
    9223372036854775783,
    18446744073709551557,
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_grisu2_finite_bit_patterns() {
        let records: Vec<serde_json::Value> =
            serde_json::from_slice(include_bytes!("../../tests/fixtures/float-format.json"))
                .unwrap();
        assert!(records.len() > 1900);
        for record in records {
            let v = f64::from_bits(record["bits"].as_u64().unwrap());
            assert_eq!(
                format_double(v),
                record["text"].as_str().unwrap(),
                "bits={:016x}",
                v.to_bits()
            );
        }
        assert_eq!(format_double(-0.0), "-0.0");
    }
}
