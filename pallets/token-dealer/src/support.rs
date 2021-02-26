use sp_runtime::traits::{CheckedConversion, Convert};
use sp_std::{
	collections::btree_set::BTreeSet,
	convert::{TryFrom, TryInto},
	marker::PhantomData,
	prelude::*,
	result,
};
use xcm::v0::{Error, Junction, MultiAsset, MultiLocation, Result};
use xcm_executor::traits::{
	FilterAssetLocation, LocationConversion, MatchesFungible, NativeAsset, TransactAsset,
};

use frame_support::{
	debug,
	traits::{Currency, ExistenceRequirement, Get, WithdrawReasons},
};

type TokenId = Vec<u8>;

#[derive(sp_runtime::RuntimeDebug)]
pub enum CurrencyId {
	/// The local instance of `balances` pallet
	Native,
	/// Token registered in `token-factory` pallet
	Token(TokenId),
}

impl From<CurrencyId> for Vec<u8> {
	fn from(other: CurrencyId) -> Vec<u8> {
		match other {
			CurrencyId::Native => b"GLMR".to_vec(),
			CurrencyId::Token(t) => t,
		}
	}
}

pub trait CurrencyIdConversion<CurrencyId> {
	fn from_asset(asset: &MultiAsset) -> Option<CurrencyId>;
}

pub struct CurrencyAdapter<
	NativeCurrency,
	TokenFactory,
	Matcher,
	AccountIdConverter,
	AccountId,
	CurrencyIdConverter,
	CurrencyId,
>(
	PhantomData<(
		NativeCurrency,
		TokenFactory,
		Matcher,
		AccountIdConverter,
		AccountId,
		CurrencyIdConverter,
		CurrencyId,
	)>,
);

impl<
		NativeCurrency: Currency<AccountId>,
		TokenFactory: token_factory::TokenFactory<Vec<u8>, AccountId, NativeCurrency::Balance>,
		Matcher: MatchesFungible<NativeCurrency::Balance>,
		AccountIdConverter: LocationConversion<AccountId>,
		AccountId: sp_std::fmt::Debug,
		CurrencyIdConverter: CurrencyIdConversion<CurrencyId>,
	> TransactAsset
	for CurrencyAdapter<
		NativeCurrency,
		TokenFactory,
		Matcher,
		AccountIdConverter,
		AccountId,
		CurrencyIdConverter,
		CurrencyId,
	>
{
	fn deposit_asset(asset: &MultiAsset, location: &MultiLocation) -> Result {
		debug::info!("------------------------------------------------");
		debug::info!(
			">>> trying deposit. asset: {:?}, location: {:?}",
			asset,
			location
		);
		let who = AccountIdConverter::from_location(location).ok_or(())?;
		debug::info!("who: {:?}", who);
		let currency = CurrencyIdConverter::from_asset(asset).ok_or(())?;
		debug::info!("currency_id: {:?}", currency);
		let amount: NativeCurrency::Balance = Matcher::matches_fungible(&asset).ok_or(())?;
		debug::info!("amount: {:?}", amount);
		// match on currency variant
		if let CurrencyId::Token(token_id) = currency {
			// mint erc20 token to `who`
			TokenFactory::mint(token_id, who, amount).map_err(|_| ())?;
		} else {
			// native currency transfer via `frame/pallet_balances` is only other variant
			NativeCurrency::deposit_creating(&who, amount);
		}
		debug::info!(">>> successful deposit.");
		debug::info!("------------------------------------------------");
		Ok(())
	}

	fn withdraw_asset(
		asset: &MultiAsset,
		location: &MultiLocation,
	) -> result::Result<MultiAsset, Error> {
		debug::info!("------------------------------------------------");
		debug::info!(
			">>> trying withdraw. asset: {:?}, location: {:?}",
			asset,
			location
		);
		let who = AccountIdConverter::from_location(location).ok_or(())?;
		debug::info!("who: {:?}", who);
		let currency = CurrencyIdConverter::from_asset(asset).ok_or(())?;
		debug::info!("currency_id: {:?}", currency);
		let amount: NativeCurrency::Balance = Matcher::matches_fungible(&asset).ok_or(())?;
		debug::info!("amount: {:?}", amount);
		// match on currency variant
		if let CurrencyId::Token(token_id) = currency {
			// burn erc20 token from `who`
			TokenFactory::burn(token_id, who, amount).map_err(|_| ())?;
		} else {
			// native currency transfer via `frame/pallet_balances` is only other variant
			NativeCurrency::withdraw(
				&who,
				amount,
				WithdrawReasons::TRANSFER,
				ExistenceRequirement::AllowDeath,
			)
			.map_err(|_| ())?;
		}
		debug::info!(">>> successful withdraw.");
		debug::info!("------------------------------------------------");
		Ok(asset.clone())
	}
}

pub struct IsConcreteWithGeneralKey<CurrencyId, FromRelayChainBalance>(
	PhantomData<(CurrencyId, FromRelayChainBalance)>,
);
impl<CurrencyId, B, FromRelayChainBalance> MatchesFungible<B>
	for IsConcreteWithGeneralKey<CurrencyId, FromRelayChainBalance>
where
	CurrencyId: TryFrom<Vec<u8>>,
	B: TryFrom<u128>,
	FromRelayChainBalance: Convert<u128, u128>,
{
	fn matches_fungible(a: &MultiAsset) -> Option<B> {
		if let MultiAsset::ConcreteFungible { id, amount } = a {
			if id == &MultiLocation::X1(Junction::Parent) {
				// Convert relay chain decimals to local chain
				let local_amount = FromRelayChainBalance::convert(*amount);
				return CheckedConversion::checked_from(local_amount);
			}
			if let Some(Junction::GeneralKey(key)) = id.last() {
				if TryInto::<CurrencyId>::try_into(key.clone()).is_ok() {
					return CheckedConversion::checked_from(*amount);
				}
			}
		}
		None
	}
}

pub struct NativePalletAssetOr<Pairs>(PhantomData<Pairs>);
impl<Pairs: Get<BTreeSet<(Vec<u8>, MultiLocation)>>> FilterAssetLocation
	for NativePalletAssetOr<Pairs>
{
	fn filter_asset_location(asset: &MultiAsset, origin: &MultiLocation) -> bool {
		if NativeAsset::filter_asset_location(asset, origin) {
			return true;
		}

		// native token
		if let MultiAsset::ConcreteFungible { ref id, .. } = asset {
			if let Some(Junction::GeneralKey(key)) = id.last() {
				return Pairs::get().contains(&(key.clone(), origin.clone()));
			}
		}

		false
	}
}

pub struct CurrencyIdConverter<CurrencyId, RelayChainCurrencyId>(
	PhantomData<CurrencyId>,
	PhantomData<RelayChainCurrencyId>,
);
impl<CurrencyId, RelayChainCurrencyId> CurrencyIdConversion<CurrencyId>
	for CurrencyIdConverter<CurrencyId, RelayChainCurrencyId>
where
	CurrencyId: TryFrom<Vec<u8>>,
	RelayChainCurrencyId: Get<CurrencyId>,
{
	fn from_asset(asset: &MultiAsset) -> Option<CurrencyId> {
		if let MultiAsset::ConcreteFungible { id: location, .. } = asset {
			if location == &MultiLocation::X1(Junction::Parent) {
				return Some(RelayChainCurrencyId::get());
			}
			if let Some(Junction::GeneralKey(key)) = location.last() {
				return CurrencyId::try_from(key.clone()).ok();
			}
		}
		None
	}
}
