// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::api::scalars::big_int::BigInt;
use async_graphql::SimpleObject;
use sui_types::sui_system_state::sui_system_state_inner_v1::StorageFundV1;

/// SUI set aside to account for objects stored on-chain.
#[derive(Clone, Debug, PartialEq, Eq, SimpleObject)]
pub(crate) struct StorageFund {
    /// Sum of storage rebates of live objects on chain.
    pub total_object_storage_rebates: Option<BigInt>,

    /// The portion of the storage fund that will never be refunded through storage rebates.
    /// The system maintains an invariant that the sum of all storage fees into the storage fund is equal to the sum of all storage rebates out, the total storage rebates remaining, and the non-refundable balance.
    pub non_refundable_balance: Option<BigInt>,
}

impl From<StorageFundV1> for StorageFund {
    fn from(value: StorageFundV1) -> Self {
        StorageFund {
            total_object_storage_rebates: Some(value.total_object_storage_rebates.value().into()),
            non_refundable_balance: Some(value.non_refundable_balance.value().into()),
        }
    }
}
