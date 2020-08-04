extern crate chrono;

#[macro_use]
pub mod macros;
pub mod details;
pub mod summary;

use crate::models::backend::transactions::{
     ModuleTransaction, MultisigTransaction,
};
use crate::models::commons::Operation;
use crate::models::service::transactions::{
    Custom, Erc20Transfer, Erc721Transfer, EtherTransfer, SettingsChange, TransactionInfo,
    TransactionStatus, Transfer, TransferInfo,
};
use crate::providers::info::{InfoProvider, SafeInfo, TokenInfo, TokenType};

impl MultisigTransaction {
    fn confirmation_count(&self) -> u64 {
        match &self.confirmations {
            Some(confirmations) => confirmations.len() as u64,
            None => 0,
        }
    }

    fn confirmation_required(&self, threshold: u64) -> u64 {
        self.confirmations_required.unwrap_or(threshold)
    }

    fn map_status(&self, safe_info: &SafeInfo) -> TransactionStatus {
        if self.is_executed {
            if self.is_successful.unwrap_or(false) {
                TransactionStatus::Success
            } else {
                TransactionStatus::Failed
            }
        } else if safe_info.nonce > self.nonce {
            TransactionStatus::Cancelled
        } else if self.confirmation_count() < self.confirmation_required(safe_info.threshold) {
            TransactionStatus::AwaitingConfirmations
        } else {
            TransactionStatus::AwaitingExecution
        }
    }

    fn transaction_info(&self, info_provider: &mut InfoProvider) -> TransactionInfo {
        if self.is_settings_change() {
            // Early exit if it is a setting change
            return TransactionInfo::SettingsChange(self.to_settings_change());
        }
        let token = info_provider.token_info(&self.to).ok();
        if self.is_erc20_transfer(&token) {
            TransactionInfo::Transfer(self.to_erc20_transfer(&token.as_ref().unwrap()))
        } else if self.is_erc721_transfer(&token) {
            TransactionInfo::Transfer(self.to_erc721_transfer(&token.as_ref().unwrap()))
        } else if self.is_ether_transfer() {
            TransactionInfo::Transfer(self.to_ether_transfer())
        } else {
            TransactionInfo::Custom(self.to_custom())
        }
    }

    fn is_erc20_transfer(&self, token: &Option<TokenInfo>) -> bool {
        self.operation.contains(&Operation::CALL)
            && token
                .as_ref()
                .and_then(|t| Some(matches!(t.token_type, TokenType::Erc20)))
                .unwrap_or(false)
            && self.data_decoded.is_some()
            && self
                .data_decoded
                .as_ref()
                .unwrap()
                .is_erc20_transfer_method()
    }

    fn is_erc721_transfer(&self, token: &Option<TokenInfo>) -> bool {
        self.operation.contains(&Operation::CALL)
            && token
                .as_ref()
                .and_then(|t| Some(matches!(t.token_type, TokenType::Erc721)))
                .unwrap_or(false)
            && self.data_decoded.is_some()
            && self
                .data_decoded
                .as_ref()
                .unwrap()
                .is_erc721_transfer_method()
    }

    fn is_ether_transfer(&self) -> bool {
        self.operation.contains(&Operation::CALL)
            && self.data.is_none()
            && self
                .value
                .as_ref()
                .and_then(|v| Some(v.parse().unwrap_or(0) > 0))
                .unwrap_or(false)
    }

    fn is_settings_change(&self) -> bool {
        self.to == self.safe
            && self.operation.contains(&Operation::CALL)
            && self.data_decoded.is_some()
            && self.data_decoded.as_ref().unwrap().is_settings_change()
    }

    fn to_erc20_transfer(&self, token: &TokenInfo) -> Transfer {
        Transfer {
            sender: self.safe.to_owned(),
            recipient: self
                .data_decoded
                .as_ref()
                .and_then(|it| it.get_parameter_value("to"))
                .unwrap_or(String::from("0x0")),
            transfer_info: TransferInfo::Erc20(Erc20Transfer {
                token_address: token.address.to_owned(),
                logo_uri: token.logo_uri.to_owned(),
                token_name: Some(token.name.to_owned()),
                token_symbol: Some(token.symbol.to_owned()),
                decimals: Some(token.decimals),
                value: self
                    .data_decoded
                    .as_ref()
                    .and_then(|it| it.get_parameter_value("value"))
                    .unwrap_or(String::from("0")),
            }),
        }
    }

    fn to_erc721_transfer(&self, token: &TokenInfo) -> Transfer {
        Transfer {
            sender: self.safe.to_owned(),
            recipient: self
                .data_decoded
                .as_ref()
                .and_then(|it| match it.get_parameter_value("_to") {
                    Some(e) => Some(e),
                    None => it.get_parameter_value("to"),
                })
                .unwrap_or(String::from("0x0")),
            transfer_info: TransferInfo::Erc721(Erc721Transfer {
                token_address: token.address.to_owned(),
                token_name: Some(token.name.to_owned()),
                token_symbol: Some(token.symbol.to_owned()),
                token_id: self
                    .data_decoded
                    .as_ref()
                    .and_then(|it| match it.get_parameter_value("tokenId") {
                        Some(e) => Some(e),
                        None => it.get_parameter_value("value"),
                    })
                    .unwrap_or(String::from("0")),
                logo_uri: token.logo_uri.to_owned(),
            }),
        }
    }

    fn to_ether_transfer(&self) -> Transfer {
        Transfer {
            sender: self.safe.to_owned(),
            recipient: self.to.to_owned(),
            transfer_info: TransferInfo::Ether(EtherTransfer {
                value: self.value.as_ref().unwrap().to_string(),
            }),
        }
    }

    fn to_settings_change(&self) -> SettingsChange {
        SettingsChange {
            data_decoded: self.data_decoded.as_ref().unwrap().to_owned(),
        }
    }

    fn to_custom(&self) -> Custom {
        Custom {
            to: self.to.to_owned(),
            data_size: data_size(&self.data),
            value: self.value.as_ref().unwrap().into(),
        }
    }
}

impl ModuleTransaction {
    fn to_transaction_info(&self) -> TransactionInfo {
        TransactionInfo::Custom(Custom {
            to: self.to.to_owned(),
            data_size: data_size(&self.data),
            value: self.value.as_ref().unwrap_or(&String::from("0")).clone(),
        })
    }
}

fn data_size(data: &Option<String>) -> String {
    match data {
        Some(actual_data) => {
            let length = actual_data.len();
            match length {
                0 => 0,
                _ => (length - 2),
            }
        }
        None => 0,
    }
    .to_string()
}