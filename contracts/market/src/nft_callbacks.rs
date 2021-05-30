use crate::*;

/// approval callbacks from NFT Contracts

trait NonFungibleTokenApprovalsReceiver {
    fn nft_on_approve(
        &mut self,
        token_id: TokenId,
        owner_id: ValidAccountId,
        approval_id: U64,
        msg: String,
    );
}

#[near_bindgen]
impl NonFungibleTokenApprovalsReceiver for Contract {
    /// where we add the sale because we know nft owner can only call nft_approve

    fn nft_on_approve(
        &mut self,
        token_id: TokenId,
        owner_id: ValidAccountId,
        approval_id: U64,
        msg: String,
    ) {
        let owner_paid_storage = self.storage_deposits.get(owner_id.as_ref()).unwrap_or(0);
        assert!(
            owner_paid_storage >= STORAGE_PER_SALE,
            "Required minimum storage to sell on market: {}",
            STORAGE_PER_SALE
        );

        let SaleArgs {
            sale_conditions,
            token_type
        } = near_sdk::serde_json::from_str(&msg).expect("Not valid SaleArgs");

        let nft_contract_id = env::predecessor_account_id();

        let mut conditions = HashMap::new();

        for Price { price, ft_token_id } in sale_conditions {
            if !self.ft_token_ids.contains(ft_token_id.as_ref()) {
                env::panic(
                    format!("Token {} not supported by this market", ft_token_id).as_bytes(),
                );
            }
            // sale is denominated in FT or 0 if accepting bids
            conditions.insert(ft_token_id.into(), price.unwrap_or(U128(0)));
        }

        // log!("add_sale for owner: {}", &owner_id);

        let contract_and_token_id = format!("{}{}{}", nft_contract_id, DELIMETER, token_id);
        self.sales.insert(
            &contract_and_token_id,
            &Sale {
                owner_id: owner_id.clone().into(),
                created_at: env::block_timestamp().into(),
                approval_id,
                nft_contract_id: nft_contract_id.clone(),
                token_id: token_id.clone(),
                conditions,
                is_series: None,
                token_type: token_type.clone(),
                bids: None,
            },
        );

        // extra for views

        let mut by_owner_id = self.by_owner_id.get(owner_id.as_ref()).unwrap_or_else(|| {
            UnorderedSet::new(StorageKey::ByOwnerIdInner {
                account_id_hash: hash_account_id(owner_id.as_ref()),
            })
        });

        let owner_occupied_storage = u128::from(by_owner_id.len()) * STORAGE_PER_SALE;
        assert!(
            owner_paid_storage > owner_occupied_storage,
            "User has more sales than storage paid"
        );
        by_owner_id.insert(&contract_and_token_id);
        self.by_owner_id.insert(owner_id.as_ref(), &by_owner_id);

        let mut by_nft_contract_id = self
            .by_nft_contract_id
            .get(&nft_contract_id)
            .unwrap_or_else(|| {
                UnorderedSet::new(StorageKey::ByNFTContractIdInner {
                    account_id_hash: hash_account_id(&nft_contract_id),
                })
            });
        by_nft_contract_id.insert(&contract_and_token_id);
        self.by_nft_contract_id
            .insert(&nft_contract_id, &by_nft_contract_id);

        if let Some(token_type) = token_type {
            assert!(
                token_id.contains(&token_type),
                "TokenType should be substr of TokenId"
            );
            let mut by_nft_token_type =
                self.by_nft_token_type.get(&token_type).unwrap_or_else(|| {
                    UnorderedSet::new(
                        StorageKey::ByNFTTokenTypeInner {
                            token_type_hash: hash_account_id(&token_type),
                        }
                    )
                });
            by_nft_token_type.insert(&contract_and_token_id);
            self.by_nft_token_type
                .insert(&token_type, &by_nft_token_type);
        }
    }
}

trait NonFungibleSeriesApprovalReceiver {
    fn series_on_approve(&mut self, series_name: String, owner_id: ValidAccountId, msg: SaleArgs);
}

#[near_bindgen]
impl NonFungibleSeriesApprovalReceiver for Contract {
    #[payable]
    fn series_on_approve(&mut self, series_name: String, owner_id: ValidAccountId, msg: SaleArgs) {
        // pay storage for 1 sale listing
        let storage_amount = self.storage_amount().0;
        self.storage_deposit(Some(owner_id.clone()), Some(storage_amount));
        // refund excess
        Promise::new(owner_id.clone().into())
            .transfer(env::attached_deposit().saturating_sub(storage_amount));

        // double check owner has enough storage for market listing
        let owner_paid_storage = self.storage_deposits.get(owner_id.as_ref()).unwrap_or(0);
        assert!(
            owner_paid_storage >= STORAGE_PER_SALE,
            "Required minimum storage to sell on market: {}",
            STORAGE_PER_SALE
        );

        let nft_contract_id = env::predecessor_account_id();
        let sale_conditions = msg.sale_conditions;

        let mut conditions = HashMap::new();
        for Price { price, ft_token_id } in sale_conditions {
            if !self.ft_token_ids.contains(ft_token_id.as_ref()) {
                env::panic(
                    format!("Token {} not supported by this market", ft_token_id).as_bytes(),
                );
            }
            conditions.insert(ft_token_id.into(), price.unwrap_or(U128(0)));
        }

        // log!("add_sale for owner: {}", &owner_id);

        let contract_and_token_id = format!("{}{}{}", nft_contract_id, DELIMETER, series_name);
        self.sales.insert(
            &contract_and_token_id,
            &Sale {
                owner_id: owner_id.clone().into(),
                created_at: env::block_timestamp().into(),
                approval_id: U64(0),
                nft_contract_id: nft_contract_id.clone(),
                token_id: series_name.clone(),
                conditions,
                is_series: Some(true),
                token_type: None,
                bids: None,
            },
        );

        // extra for views

        let mut by_owner_id = self.by_owner_id.get(owner_id.as_ref()).unwrap_or_else(|| {
            UnorderedSet::new(
                StorageKey::ByOwnerIdInner {
                    account_id_hash: hash_account_id(owner_id.as_ref()),
                }
            )
        });

        let owner_occupied_storage = u128::from(by_owner_id.len()) * STORAGE_PER_SALE;
        assert!(
            owner_paid_storage > owner_occupied_storage,
            "User has more sales than storage paid"
        );
        by_owner_id.insert(&contract_and_token_id);
        self.by_owner_id.insert(owner_id.as_ref(), &by_owner_id);

        let mut by_nft_contract_id = self
            .by_nft_contract_id
            .get(&nft_contract_id)
            .unwrap_or_else(|| {
                UnorderedSet::new(
                    StorageKey::ByNFTContractIdInner {
                        account_id_hash: hash_account_id(&nft_contract_id),
                    }
                )
            });
        by_nft_contract_id.insert(&contract_and_token_id);
        self.by_nft_contract_id
            .insert(&nft_contract_id, &by_nft_contract_id);

        let mut by_nft_token_type = self.by_nft_token_type.get(&series_name).unwrap_or_else(|| {
            UnorderedSet::new(
                StorageKey::ByNFTTokenTypeInner {
                    token_type_hash: hash_account_id(&series_name),
                }
            )
        });
        by_nft_token_type.insert(&contract_and_token_id);
        self.by_nft_token_type
            .insert(&series_name, &by_nft_token_type);
    }
}
