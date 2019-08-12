// Copyright 2015-2017 Parity Technologies (UK) Ltd.
// This file is part of Parity.

// This software is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This software is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

//! Receipt

use super::Bytes;
use std::str::FromStr;

use crate::block_number::BlockNumber;
use crate::errors::ReceiptError;
use crate::log_entry::{LocalizedLogEntry, LogBloom, LogEntry};

use cita_types::traits::LowerHex;
use cita_types::{Address, H256, U256};
use jsonrpc_types::rpc_types::Receipt as RpcReceipt;
use libproto::executor::{Receipt as ProtoReceipt, ReceiptErrorWithOption, StateRoot};
use rlp::{Decodable, DecoderError, Encodable, RlpStream, UntrustedRlp};

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    pub state_root: Option<H256>,
    pub quota_used: U256,
    pub log_bloom: LogBloom,
    pub logs: Vec<LogEntry>,
    pub error: Option<ReceiptError>,
    pub account_nonce: U256,
    pub transaction_hash: H256,
}

impl Receipt {
    pub fn new(
        state_root: Option<H256>,
        quota_used: U256,
        logs: Vec<LogEntry>,
        error: Option<ReceiptError>,
        account_nonce: U256,
        transaction_hash: H256,
    ) -> Receipt {
        Receipt {
            state_root,
            quota_used,
            log_bloom: logs.iter().fold(LogBloom::default(), |b, l| b | l.bloom()),
            logs,
            error,
            account_nonce,
            transaction_hash,
        }
    }

    pub fn protobuf(&self) -> ProtoReceipt {
        let mut receipt_proto = ProtoReceipt::new();
        let mut state_root_option = StateRoot::new();
        let mut receipt_error_with_option = ReceiptErrorWithOption::new();

        if let Some(state_root) = self.state_root {
            state_root_option.set_state_root(state_root.to_vec());
            receipt_proto.set_state_root(state_root_option);
        }

        if let Some(error) = self.error {
            receipt_error_with_option.set_error(error.protobuf());
            receipt_proto.set_error(receipt_error_with_option);
        }

        receipt_proto.set_quota_used(self.quota_used.lower_hex());
        receipt_proto.set_log_bloom(self.log_bloom.to_vec());
        receipt_proto.logs = self
            .logs
            .clone()
            .into_iter()
            .map(|log_entry| log_entry.protobuf())
            .collect();
        receipt_proto.set_account_nonce(self.account_nonce.as_u64());
        receipt_proto.set_transaction_hash(self.transaction_hash.to_vec());
        receipt_proto
    }
}

impl From<ProtoReceipt> for Receipt {
    fn from(receipt: ProtoReceipt) -> Self {
        let state_root = if receipt.state_root.is_some() {
            Some(H256::from_slice(
                receipt.clone().take_state_root().get_state_root(),
            ))
        } else {
            None
        };

        let quota_used: U256 = U256::from_str(receipt.get_quota_used()).unwrap();
        let account_nonce: U256 = U256::from(receipt.get_account_nonce());
        let transaction_hash: H256 = H256::from_slice(receipt.get_transaction_hash());
        let mut error = None;

        let logs = receipt
            .get_logs()
            .iter()
            .map(|log_entry| {
                let address: Address = Address::from_slice(log_entry.get_address());
                let topics: Vec<H256> = log_entry
                    .get_topics()
                    .iter()
                    .map(|topic| H256::from_slice(topic))
                    .collect();
                let data: Bytes = Bytes::from(log_entry.get_data());
                LogEntry {
                    address,
                    topics,
                    data,
                }
            })
            .collect();

        if receipt.error.is_some() {
            error = Some(ReceiptError::from_proto(
                receipt.clone().take_error().get_error(),
            ));
        }

        Receipt::new(
            state_root,
            quota_used,
            logs,
            error,
            account_nonce,
            transaction_hash,
        )
    }
}

impl Encodable for Receipt {
    fn rlp_append(&self, s: &mut RlpStream) {
        if let Some(ref root) = self.state_root {
            s.begin_list(7);
            s.append(root);
        } else {
            s.begin_list(6);
        }
        s.append(&self.quota_used);
        s.append(&self.log_bloom);
        s.append_list(&self.logs);
        s.append(&self.error);
        s.append(&self.account_nonce);
        s.append(&self.transaction_hash);
    }
}

impl Decodable for Receipt {
    fn decode(rlp: &UntrustedRlp) -> Result<Self, DecoderError> {
        if rlp.item_count()? == 6 {
            Ok(Receipt {
                state_root: None,
                quota_used: rlp.val_at(0)?,
                log_bloom: rlp.val_at(1)?,
                logs: rlp.list_at(2)?,
                error: rlp.val_at(3)?,
                account_nonce: rlp.val_at(4)?,
                transaction_hash: rlp.val_at(5)?,
            })
        } else {
            Ok(Receipt {
                state_root: Some(rlp.val_at(0)?),
                quota_used: rlp.val_at(1)?,
                log_bloom: rlp.val_at(2)?,
                logs: rlp.list_at(3)?,
                error: rlp.val_at(4)?,
                account_nonce: rlp.val_at(5)?,
                transaction_hash: rlp.val_at(6)?,
            })
        }
    }
}

#[derive(Debug, Clone)]
pub struct RichReceipt {
    pub transaction_hash: H256,
    pub transaction_index: usize,
    pub block_hash: H256,
    pub block_number: BlockNumber,
    pub cumulative_quota_used: U256,
    pub quota_used: U256,
    pub contract_address: Option<Address>,
    pub logs: Vec<LocalizedLogEntry>,
    pub log_bloom: LogBloom,
    pub state_root: Option<H256>,
    pub error: Option<ReceiptError>,
}

impl Into<RpcReceipt> for RichReceipt {
    fn into(self) -> RpcReceipt {
        RpcReceipt {
            transaction_hash: Some(self.transaction_hash),
            transaction_index: Some(self.transaction_index.into()),
            block_hash: Some(self.block_hash),
            block_number: Some(self.block_number.into()),
            cumulative_quota_used: self.cumulative_quota_used,
            quota_used: Some(self.quota_used),
            contract_address: self.contract_address.map(Into::into),
            logs: self.logs.into_iter().map(Into::into).collect(),
            state_root: self.state_root.map(Into::into),
            logs_bloom: self.log_bloom,
            error_message: self.error.map(ReceiptError::description),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log_entry::LogEntry;

    #[test]
    fn test_no_state_root() {
        let r = Receipt::new(
            None,
            0x40cae.into(),
            vec![LogEntry {
                address: "dcf421d093428b096ca501a7cd1a740855a7976f".into(),
                topics: vec![],
                data: vec![0u8; 32],
            }],
            None,
            1.into(),
            "2f697d671e9ae4ee24a43c4b0d7e15f1cb4ba6de1561120d43b9a4e8c4a8a6ee".into(),
        );
        let encoded = ::rlp::encode(&r);
        println!("encode ok");
        let decoded: Receipt = ::rlp::decode(&encoded);
        println!("decoded: {:?}", decoded);
        assert_eq!(decoded, r);
    }

    #[test]
    fn test_basic() {
        let r = Receipt::new(
            Some("2f697d671e9ae4ee24a43c4b0d7e15f1cb4ba6de1561120d43b9a4e8c4a8a6ee".into()),
            0x40cae.into(),
            vec![LogEntry {
                address: "dcf421d093428b096ca501a7cd1a740855a7976f".into(),
                topics: vec![],
                data: vec![0u8; 32],
            }],
            None,
            1.into(),
            "2f697d671e9ae4ee24a43c4b0d7e15f1cb4ba6de1561120d43b9a4e8c4a8a6ee".into(),
        );
        let encoded = ::rlp::encode(&r);
        let decoded: Receipt = ::rlp::decode(&encoded);
        println!("decoded: {:?}", decoded);
        assert_eq!(decoded, r);
    }

    #[test]
    fn test_with_error() {
        let r = Receipt::new(
            Some("2f697d671e9ae4ee24a43c4b0d7e15f1cb4ba6de1561120d43b9a4e8c4a8a6ee".into()),
            0x40cae.into(),
            vec![LogEntry {
                address: "dcf421d093428b096ca501a7cd1a740855a7976f".into(),
                topics: vec![],
                data: vec![0u8; 32],
            }],
            Some(ReceiptError::NoTransactionPermission),
            1.into(),
            "2f697d671e9ae4ee24a43c4b0d7e15f1cb4ba6de1561120d43b9a4e8c4a8a6ee".into(),
        );
        let encoded = ::rlp::encode(&r);
        let decoded: Receipt = ::rlp::decode(&encoded);
        println!("decoded: {:?}", decoded);
        assert_eq!(decoded, r);
    }
}
