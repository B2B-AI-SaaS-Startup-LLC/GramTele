//! Literal builtin TLS rule tree from PrepareClientHelloRules.
use tg_scheme::*;
pub fn prepare_client_hello_rules() -> Vec<TlsBlock> {
    vec![
        mtp_tls_block_string(vec![22, 3, 1]),
        mtp_tls_block_scope(vec![
            mtp_tls_block_string(vec![1, 0]),
            mtp_tls_block_scope(vec![
                mtp_tls_block_string(vec![3, 3]),
                mtp_tls_block_zero(32),
                mtp_tls_block_string(vec![32]),
                mtp_tls_block_random(32),
                mtp_tls_block_string(vec![0, 32]),
                mtp_tls_block_grease(0),
                mtp_tls_block_string(vec![
                    19, 1, 19, 2, 19, 3, 192, 43, 192, 47, 192, 44, 192, 48, 204, 169, 204, 168,
                    192, 19, 192, 20, 0, 156, 0, 157, 0, 47, 0, 53, 1, 0,
                ]),
                mtp_tls_block_scope(vec![
                    mtp_tls_block_grease(2),
                    mtp_tls_block_string(vec![0, 0]),
                    mtp_tls_block_permutation(vec![
                        vec![
                            mtp_tls_block_string(vec![0, 0]),
                            mtp_tls_block_scope(vec![mtp_tls_block_scope(vec![
                                mtp_tls_block_string(vec![0]),
                                mtp_tls_block_scope(vec![mtp_tls_block_domain()]),
                            ])]),
                        ],
                        vec![mtp_tls_block_string(vec![0, 5, 0, 5, 1, 0, 0, 0, 0])],
                        vec![
                            mtp_tls_block_string(vec![0, 10, 0, 12, 0, 10]),
                            mtp_tls_block_grease(4),
                            mtp_tls_block_string(vec![17, 236, 0, 29, 0, 23, 0, 24]),
                        ],
                        vec![mtp_tls_block_string(vec![0, 11, 0, 2, 1, 0])],
                        vec![mtp_tls_block_string(vec![
                            0, 13, 0, 24, 0, 22, 9, 4, 9, 5, 9, 6, 4, 3, 8, 4, 4, 1, 5, 3, 8, 5, 5,
                            1, 8, 6, 6, 1,
                        ])],
                        vec![mtp_tls_block_string(vec![
                            0, 16, 0, 14, 0, 12, 2, 104, 50, 8, 104, 116, 116, 112, 47, 49, 46, 49,
                        ])],
                        vec![mtp_tls_block_string(vec![0, 18, 0, 0])],
                        vec![mtp_tls_block_string(vec![0, 23, 0, 0])],
                        vec![mtp_tls_block_string(vec![0, 27, 0, 3, 2, 0, 2])],
                        vec![mtp_tls_block_string(vec![0, 35, 0, 0])],
                        vec![
                            mtp_tls_block_string(vec![0, 43, 0, 7, 6]),
                            mtp_tls_block_grease(6),
                            mtp_tls_block_string(vec![3, 4, 3, 3]),
                        ],
                        vec![mtp_tls_block_string(vec![0, 45, 0, 2, 1, 1])],
                        vec![
                            mtp_tls_block_string(vec![0, 51, 4, 239, 4, 237]),
                            mtp_tls_block_grease(4),
                            mtp_tls_block_string(vec![0, 1, 0, 17, 236, 4, 192]),
                            mtp_tls_block_m(),
                            mtp_tls_block_public_key(),
                            mtp_tls_block_string(vec![0, 29, 0, 32]),
                            mtp_tls_block_public_key(),
                        ],
                        vec![mtp_tls_block_string(vec![68, 205, 0, 5, 0, 3, 2, 104, 50])],
                        vec![
                            mtp_tls_block_string(vec![254, 13]),
                            mtp_tls_block_scope(vec![
                                mtp_tls_block_string(vec![0, 0, 1, 0, 1]),
                                mtp_tls_block_random(1),
                                mtp_tls_block_string(vec![0, 32]),
                                mtp_tls_block_public_key(),
                                mtp_tls_block_scope(vec![mtp_tls_block_e()]),
                            ]),
                        ],
                        vec![mtp_tls_block_string(vec![255, 1, 0, 1, 0])],
                    ]),
                    mtp_tls_block_grease(3),
                    mtp_tls_block_string(vec![0, 1, 0]),
                    mtp_tls_block_padding(),
                ]),
            ]),
        ]),
    ]
}
