// Copyright 2019-2022 PureStake Inc.
// This file is part of Moonbeam.

// Moonbeam is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Moonbeam is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Moonbeam.  If not, see <http://www.gnu.org/licenses/>.

//! Implements RANDAO instances to enable incentivized, collective RNGs

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::pallet;

pub use pallet::*;

// pub mod weights;
// use weights::WeightInfo;
// #[cfg(any(test, feature = "runtime-benchmarks"))]
// mod benchmarks;
// #[cfg(test)]
// mod mock;
// #[cfg(test)]
// mod tests;

#[pallet]
pub mod pallet {
	// use crate::WeightInfo;
	use frame_support::pallet_prelude::*;
	use frame_support::traits::{Currency, Member, ReservableCurrency};
	use frame_system::pallet_prelude::*;
    use scale_info::TypeInfo;
    use sp_runtime::Percent;

	pub type BalanceOf<T> = <<T as Config>::Currency as Currency<
		<T as frame_system::Config>::AccountId,
	>>::Balance;

	#[derive(Encode, Decode, PartialEq, Eq, Debug, TypeInfo)]
    // TODO: how is randao paid? where do the fee payments go?
	pub(crate) struct RandaoInfo<AccountId, Balance, BlockNumber> {
        /// Account in charge of adding/removing members to contribute secret numbers
        /// This account also can configure the parameters to make source of randomness competitive
		pub(crate) coordinator: AccountId,
        /// Fee required to generate a random number, parameter used for smart contract using this
        /// source of randomness
        pub(crate) fee: Balance,
        /// Required starting deposit for all members of the group
		pub(crate) deposit: Balance,
        /// Percent of deposit slashed if member does not participate in `commit` phase
        pub(crate) absent_penalty: Percent,
        /// Percent of deposit slashed if member does not participate in `reveal` phase
        pub(crate) commit_no_reveal_penalty: Percent,
        /// Maximum number of blocks after request starts when participant can make commitment
        pub(crate) commitment_delay: BlockNumber,
        /// Maximum number of blocks after commitment period ends when participant can reveal
        pub(crate) reveal_delay: BlockNumber,
	}

    #[derive(Encode, Decode, PartialEq, Debug, TypeInfo)]
    /// Information regarding when the randomness request started (and commitment started)
    pub(crate) struct RequestInfo<AccountId, GroupId, BlockNumber> {
        /// Contract requesting randomness
        pub(crate) requester: AccountId,
        /// Every request exactly 1 group
        pub(crate) group: GroupId,
        /// Block at which the commitment started for this randomness request
        pub(crate) started: BlockNumber,
    }

    #[derive(Encode, Decode, PartialEq, Debug, TypeInfo)]
    /// Record of participation
    pub(crate) struct Participation<Hash> {
        /// Some(commitment) if committed else None
        pub(crate) commitment: Option<Hash>,
        /// Some(secret) if revealed else None
        pub(crate) secret: Option<Hash>,
    }

	#[pallet::pallet]
	pub struct Pallet<T>(PhantomData<T>);

	/// Configuration trait of RANDAO pallet
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Overarching event type
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        /// Unique identifier for randomness requests
        type RequestId: Member;
        /// Unique identifier for RANDAO groups
        type GroupId: Member;
		/// Currency in which the security deposit will be taken.
		type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Randao group does not exist
		GroupDNE,
        /// The number of groups exceeds the capacity of the GroupId assigned type
        GroupIdOverflow,
        /// Randomness request does not exist
        RequestDNE,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// New Group Identity, Randomness Coordinator
		GroupRegistered(T::GroupId, T::AccountId),
        /// New Randomness Requested, Commitment Period Ends, Reveal Period Ends
        RandomnessRequested(T::GroupId, T::RequestId, T::BlockNumber, T::BlockNumber),
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Register group, callable only by root
		#[pallet::weight(0)]
		pub fn register_group(origin: OriginFor<T>, coordinator: T::AccountId) -> DispatchResult {
			frame_system::ensure_root(origin)?;
            let next_group_id = <GroupIdCounter<T>>::get()
                .checked_add(&1u8.into())
                .ok_or(Error::<T>::GroupIdOverflow)?;
            
            <GroupIdCounter<T>>::put(next_group_id);
			Ok(())
		}
        /// Set members
        // Request randomness
        // Trigger reveal phase
        // Fulfill randomness
	}

    #[pallet::storage]
	#[pallet::getter(fn group_id_counter)]
	/// Group Id Counter
	type GroupIdCounter<T: Config> = StorageValue<_, T::GroupId, ValueQuery>;

    #[pallet::storage]
	#[pallet::getter(fn request_id_counter)]
	/// Group Id Counter
	type GroupIdCounter<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::GroupId,
        RandaoInfo<T::AccountId, BalanceOf<T>>,
        OptionQuery,
    >;

	#[pallet::storage]
	#[pallet::getter(fn randao_group_info)]
	/// We maintain a mapping from the NimbusIds used in the consensus layer
	/// to the AccountIds runtime (including this staking pallet).
	pub type RandaoGroupInfo<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::GroupId,
		RandaoInfo<T::AccountId, BalanceOf<T>>,
		OptionQuery,
	>;

	// #[pallet::genesis_config]
	// /// Genesis config for author mapping pallet
	// pub struct GenesisConfig<T: Config> {
	// 	/// The associations that should exist at chain genesis
	// 	pub mappings: Vec<(NimbusId, T::AccountId)>,
	// }

	// #[cfg(feature = "std")]
	// impl<T: Config> Default for GenesisConfig<T> {
	// 	fn default() -> Self {
	// 		Self { mappings: vec![] }
	// 	}
	// }

	// #[pallet::genesis_build]
	// impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
	// 	fn build(&self) {
	// 		for (author_id, account_id) in &self.mappings {
	// 			if let Err(e) = Pallet::<T>::enact_registration(&author_id, &account_id) {
	// 				log::warn!("Error with genesis author mapping registration: {:?}", e);
	// 			}
	// 		}
	// 	}
	// }
}
