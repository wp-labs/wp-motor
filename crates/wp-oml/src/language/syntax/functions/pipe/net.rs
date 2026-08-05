use crate::language::prelude::*;
use num_bigint::BigUint;
use std::net::IpAddr;
use std::sync::OnceLock;

pub const PIPE_IP4_TO_INT: &str = "ip4_to_int";
pub const PIPE_IP_TO_BIGUINT: &str = "ip_to_biguint";
pub const PIPE_INTRANET_IP: &str = "intranet_ip";

/// 2^128，IPv6 统一编码偏移量。
/// IPv6 网段统一映射到 `[2^128, 2^129)`，与 IPv4 的 `[0, 2^32)` 互不重叠，
/// 两条家族共用同一条范围查询 SQL。
const IPV6_OFFSET: &str = "340282366920938463463374607431768211456";

fn two_pow_128() -> &'static BigUint {
    static TWO_POW_128: OnceLock<BigUint> = OnceLock::new();
    TWO_POW_128.get_or_init(|| IPV6_OFFSET.parse().expect("valid 2^128 constant"))
}

#[derive(Clone, Debug, Default)]
pub struct Ip4ToInt {}

impl Display for Ip4ToInt {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", PIPE_IP4_TO_INT)
    }
}

#[derive(Clone, Debug, Default)]
pub struct IpToBigUint {}

impl Display for IpToBigUint {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", PIPE_IP_TO_BIGUINT)
    }
}

/// 判断 IP 是否内网地址（返回 `内`/`外`），支持 IPv4 + IPv6，网段由配置驱动
#[derive(Clone, Debug, Default)]
pub struct IntranetIp {}

impl Display for IntranetIp {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", PIPE_INTRANET_IP)
    }
}

/// 将 IP 地址统一编码为任意精度无符号整数（`BigUint`，无精度损失）。
///
/// - IPv4：按无符号 32 位整数编码 `A×256³ + B×256² + C×256 + D`，映射到 `0 .. 4294967295`；
/// - IPv6：先解析为无符号 128 位整数，再加 `2^128`，映射到
///   `340282366920938463463374607431768211456 .. 680564733841876926926749214863536422911`。
///
/// 纯编码函数：不做字符串解析，输入 `IpAddr` 由 `std::net` 保证合法。
/// 禁止使用有符号 64 位整数与浮点数：IPv6 编码（最大 `2^129 - 1`）超出 i64/f64 精确表示范围。
pub fn ip_to_biguint(ip: IpAddr) -> BigUint {
    match ip {
        IpAddr::V4(v4) => BigUint::from(u32::from(v4)),
        IpAddr::V6(v6) => BigUint::from(u128::from(v6)) + two_pow_128(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_ip_to_biguint_acceptance() {
        // 验收用例（issue #342）
        let cases = [
            ("0.0.0.0", "0"),
            ("8.8.8.8", "134744072"),
            ("255.255.255.255", "4294967295"),
            ("::", "340282366920938463463374607431768211456"),
            (
                "2001:4860:4860::8888",
                "382824323044708348099391746388336347272",
            ),
            (
                "ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff",
                "680564733841876926926749214863536422911",
            ),
        ];
        for (input, expect) in cases {
            let ip = IpAddr::from_str(input).unwrap_or_else(|e| panic!("{input} invalid: {e}"));
            assert_eq!(
                ip_to_biguint(ip).to_string(),
                expect,
                "ip_to_biguint({input})"
            );
        }
    }

    #[test]
    fn test_ip_to_biguint_ipv6_compressed_equals_full() {
        // IPv6 压缩写法与完整写法得到相同结果
        let pairs = [
            ("::1", "0:0:0:0:0:0:0:1"),
            ("::", "0:0:0:0:0:0:0:0"),
            ("2001:4860:4860::8888", "2001:4860:4860:0:0:0:0:8888"),
            ("2001:db8::1", "2001:db8:0:0:0:0:0:1"),
        ];
        for (compressed, full) in pairs {
            let a = IpAddr::from_str(compressed).unwrap();
            let b = IpAddr::from_str(full).unwrap();
            assert_eq!(
                ip_to_biguint(a),
                ip_to_biguint(b),
                "compressed {compressed} != full {full}"
            );
        }
    }

    #[test]
    fn test_ip_to_biguint_range_union() {
        // IPv4 与 IPv6 统一键区间互不重叠：[0, 2^32) ∪ [2^128, 2^129)
        let ipv4_max = ip_to_biguint(IpAddr::from_str("255.255.255.255").unwrap());
        let ipv6_min = ip_to_biguint(IpAddr::from_str("::").unwrap());
        assert!(ipv4_max < ipv6_min);
        assert_eq!(ipv4_max, BigUint::from(4294967295u32));
        assert_eq!(ipv6_min, *two_pow_128());
    }

    #[test]
    fn test_ip_to_biguint_bounds() {
        // 区间边界：IPv6 最大值 = 2^129 - 1
        let ipv6_max =
            ip_to_biguint(IpAddr::from_str("ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff").unwrap());
        assert_eq!(ipv6_max, BigUint::from(2u8).pow(129) - BigUint::from(1u8));
    }
}
