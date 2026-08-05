pub mod prelude {
    pub use crate::assert_eq_hex;
    pub use pretty_hex::{config_hex, HexConfig};
}

#[macro_export]
macro_rules! assert_eq_hex {
    // Case when both left and right are provided, along with a config
    ($left:expr, $right:expr, $config:expr) => {
        assert_eq!(
            $left,
            $right,
            "\n\nValues don't match in hex!\nLeft:\n-------\n{} \n\nRight:\n------- \n{}\n",
            config_hex(&$left, $config),
            config_hex(&$right, $config)
        );
    };

    // Case when only left and right are provided
    ($left:expr, $right:expr) => {
        const HEX_FORMAT_CONFIG: HexConfig = HexConfig {
            title: false,
            ascii: true,
            width: 30,
            group: 0,
            chunk: 1,
            max_bytes: usize::MAX,
            display_offset: 0,
        };

        // Call the first macro clause, passing the default config
        assert_eq_hex!($left, $right, HEX_FORMAT_CONFIG)
    };
}
