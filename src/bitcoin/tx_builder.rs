use std::{cell::RefCell, rc::Rc};

use bdk_wallet::{error::CreateTxError, TxOrdering as BdkTxOrdering, Wallet as BdkWallet};
use serde::Serialize;
use wasm_bindgen::prelude::wasm_bindgen;
use std::collections::BTreeMap;
use crate::types::{Amount, BdkError, BdkErrorCode, FeeRate, OutPoint, Psbt, Recipient, ScriptBuf, KeychainKind};


/// A transaction builder.
///
/// A `TxBuilder` is created by calling [`build_tx`] or [`build_fee_bump`] on a wallet. After
/// assigning it, you set options on it until finally calling [`finish`] to consume the builder and
/// generate the transaction.
///
/// Each option setting method on `TxBuilder` takes and returns a new builder so you can chain calls
#[wasm_bindgen]
pub struct TxBuilder {
    wallet: Rc<RefCell<BdkWallet>>,
    recipients: Vec<Recipient>,
    unspendable: Vec<OutPoint>,
    fee_rate: FeeRate,
    drain_wallet: bool,
    drain_to: Option<ScriptBuf>,
    allow_dust: bool,
    ordering: TxOrdering,
    policy_paths: Vec<(KeychainKind, String, Vec<u32>)>,
}

#[wasm_bindgen]
impl TxBuilder {
    // visibile solo al crate, usato da Wallet::build_tx
    pub(crate) fn new(wallet: Rc<RefCell<BdkWallet>>) -> TxBuilder {
        TxBuilder {
            wallet,
            recipients: vec![],
            unspendable: vec![],
            fee_rate: FeeRate::new(1),
            drain_wallet: false,
            allow_dust: false,
            drain_to: None,
            ordering: BdkTxOrdering::default().into(),
            policy_paths: vec![], // <--- aggiungi questo
        }
    }

    /// Replace the recipients already added with a new list
    pub fn set_recipients(mut self, recipients: Vec<Recipient>) -> Self {
        self.recipients = recipients;
        self
    }

    /// Add a recipient to the internal list
    pub fn add_recipient(mut self, recipient: Recipient) -> Self {
        self.recipients.push(recipient);
        self
    }

    /// Replace the internal list of unspendable utxos with a new list
    pub fn unspendable(mut self, unspendable: Vec<OutPoint>) -> Self {
        self.unspendable = unspendable;
        self
    }

    /// Add a utxo to the internal list of unspendable utxos
    pub fn add_unspendable(mut self, outpoint: OutPoint) -> Self {
        self.unspendable.push(outpoint);
        self
    }

    /// Set a custom fee rate.
    pub fn fee_rate(mut self, fee_rate: FeeRate) -> Self {
        self.fee_rate = fee_rate;
        self
    }

    /// Spend all the available inputs.
    pub fn drain_wallet(mut self) -> Self {
        self.drain_wallet = true;
        self
    }

    /// Sets the address to *drain* excess coins to.
    pub fn drain_to(mut self, script_pubkey: ScriptBuf) -> Self {
        self.drain_to = Some(script_pubkey);
        self
    }

    /// Set whether or not the dust limit is checked.
    pub fn allow_dust(mut self, allow_dust: bool) -> Self {
        self.allow_dust = allow_dust;
        self
    }

    /// Choose the ordering for inputs and outputs of the transaction
    pub fn ordering(mut self, ordering: TxOrdering) -> Self {
        self.ordering = ordering;
        self
    }

    pub fn finish(self) -> Result<Psbt, BdkError> {
        let mut wallet = self.wallet.borrow_mut();
        let mut builder = wallet.build_tx();

        builder
            .ordering(self.ordering.into())
            .set_recipients(self.recipients.into_iter().map(Into::into).collect())
            .unspendable(self.unspendable.into_iter().map(Into::into).collect())
            .fee_rate(self.fee_rate.into())
            .allow_dust(self.allow_dust);

        if self.drain_wallet {
            builder.drain_wallet();
        }

        if let Some(drain_recipient) = self.drain_to {
            builder.drain_to(drain_recipient.into());
        }

        // Applica le policy path configurate da JS
        // Raggruppa tutti i nodi per keychain prima di chiamare policy_path
        let mut external_map = BTreeMap::<String, Vec<usize>>::new();
        let mut internal_map = BTreeMap::<String, Vec<usize>>::new();

        for (kc, policy_id, path_u32) in self.policy_paths.into_iter() {
            let path_usize: Vec<usize> = path_u32.into_iter().map(|v| v as usize).collect();

            match kc {
                KeychainKind::External => {
                    external_map.insert(policy_id, path_usize);
                }
                KeychainKind::Internal => {
                    internal_map.insert(policy_id, path_usize);
                }
                _ => {} // Ignora __Invalid e altri casi
            }
        }

        // Chiama policy_path una sola volta per keychain con tutti i nodi
        if !external_map.is_empty() {
            builder.policy_path(external_map, bdk_wallet::KeychainKind::External);
        }
        if !internal_map.is_empty() {
            builder.policy_path(internal_map, bdk_wallet::KeychainKind::Internal);
        }

        let psbt = builder.finish()?;
        Ok(psbt.into())
    }



    /// Imposta una spending policy path per uno specifico keychain.
    ///
    /// - `policy_id`: l'id della policy (es. quello che trovi in `wallet.policies(...)` lato JS)
    /// - `path`: il ramo scelto (array di indici `[branch_index, ...]`)
    /// - `keychain`: External / Internal
    pub fn policy_path(mut self, policy_id: String, path: Vec<u32>, keychain: KeychainKind) -> Self {
        self.policy_paths.push((keychain, policy_id, path));
        self
    }
}

/// Ordering of the transaction's inputs and outputs
#[derive(Clone, Default)]
#[wasm_bindgen]
pub enum TxOrdering {
    /// Randomized (default)
    #[default]
    Shuffle,
    /// Unchanged
    Untouched,
}

impl From<BdkTxOrdering> for TxOrdering {
    fn from(ordering: BdkTxOrdering) -> Self {
        match ordering {
            BdkTxOrdering::Shuffle => TxOrdering::Shuffle,
            BdkTxOrdering::Untouched => TxOrdering::Untouched,
            _ => panic!("Unsupported ordering"),
        }
    }
}

impl From<TxOrdering> for BdkTxOrdering {
    fn from(ordering: TxOrdering) -> Self {
        match ordering {
            TxOrdering::Shuffle => BdkTxOrdering::Shuffle,
            TxOrdering::Untouched => BdkTxOrdering::Untouched,
        }
    }
}

/// Wallet's UTXO set is not enough to cover recipient's requested plus fee.
#[wasm_bindgen]
#[derive(Clone, Serialize)]
pub struct InsufficientFunds {
    /// Amount needed for the transaction
    pub needed: Amount,
    /// Amount available for spending
    pub available: Amount,
}

impl From<CreateTxError> for BdkError {
    fn from(e: CreateTxError) -> Self {
        use CreateTxError::*;
        match &e {
            Descriptor(_) => BdkError::new(BdkErrorCode::Descriptor, e.to_string(), ()),
            Policy(_) => BdkError::new(BdkErrorCode::Policy, e.to_string(), ()),
            SpendingPolicyRequired(keychain_kind) => {
                BdkError::new(BdkErrorCode::SpendingPolicyRequired, e.to_string(), keychain_kind)
            }
            Version0 => BdkError::new(BdkErrorCode::Version0, e.to_string(), ()),
            Version1Csv => BdkError::new(BdkErrorCode::Version1Csv, e.to_string(), ()),
            LockTime { .. } => BdkError::new(BdkErrorCode::LockTime, e.to_string(), ()),
            RbfSequenceCsv { .. } => BdkError::new(BdkErrorCode::RbfSequenceCsv, e.to_string(), ()),
            FeeTooLow { required } => BdkError::new(BdkErrorCode::FeeTooLow, e.to_string(), required),
            FeeRateTooLow { required } => BdkError::new(BdkErrorCode::FeeRateTooLow, e.to_string(), required),
            NoUtxosSelected => BdkError::new(BdkErrorCode::NoUtxosSelected, e.to_string(), ()),
            OutputBelowDustLimit(limit) => BdkError::new(BdkErrorCode::OutputBelowDustLimit, e.to_string(), limit),
            CoinSelection(insufficient_funds) => BdkError::new(
                BdkErrorCode::InsufficientFunds,
                e.to_string(),
                InsufficientFunds {
                    available: insufficient_funds.available.into(),
                    needed: insufficient_funds.needed.into(),
                },
            ),
            NoRecipients => BdkError::new(BdkErrorCode::NoRecipients, e.to_string(), ()),
            Psbt(_) => BdkError::new(BdkErrorCode::Psbt, e.to_string(), ()),
            MissingKeyOrigin(_) => BdkError::new(BdkErrorCode::MissingKeyOrigin, e.to_string(), ()),
            UnknownUtxo => BdkError::new(BdkErrorCode::UnknownUtxo, e.to_string(), ()),
            MissingNonWitnessUtxo(outpoint) => {
                BdkError::new(BdkErrorCode::MissingNonWitnessUtxo, e.to_string(), outpoint)
            }
            MiniscriptPsbt(_) => BdkError::new(BdkErrorCode::MiniscriptPsbt, e.to_string(), ()),
        }
    }
}
