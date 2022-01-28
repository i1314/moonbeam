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

//! Precompile to provide source(s) of randomness
//!
//! # Collective Flip Randomness
//! <link to docs>

#![cfg_attr(not(feature = "std"), no_std)]
#![feature(assert_matches)]

use fp_evm::{Context, ExitSucceed, PrecompileOutput};
use frame_support::dispatch::{Dispatchable, GetDispatchInfo, PostDispatchInfo};
use frame_support::traits::Randomness;
use pallet_collective_flip::Call as CollectiveFlipCall;
use pallet_evm::Precompile;
use precompile_utils::{
	Bytes, EvmData, EvmDataReader, EvmDataWriter, EvmResult, FunctionModifier, Gasometer,
	RuntimeHelper,
};
use sp_core::H256;
use sp_std::{fmt::Debug, marker::PhantomData};

// #[cfg(test)]
// mod mock;
// #[cfg(test)]
// mod tests;

#[precompile_utils::generate_function_selector]
#[derive(Debug, PartialEq)]
pub enum Action {
	CollectiveFlipRandom = "collective_flip_random(bytes)",
	CollectiveFlipSeed = "collective_flip_seed",
}

/// A precompile to wrap the functionality that provides randomness
pub struct RandomnessWrapper<Runtime>(PhantomData<Runtime>);

impl<Runtime> Precompile for RandomnessWrapper<Runtime>
where
	Runtime: pallet_collective_flip::Config + pallet_evm::Config + frame_system::Config,
	Runtime::Call: Dispatchable<PostInfo = PostDispatchInfo> + GetDispatchInfo,
	<Runtime::Call as Dispatchable>::Origin: From<Option<Runtime::AccountId>>,
	Runtime::Call: From<CollectiveFlipCall<Runtime>>,
	Runtime::Hash: From<H256> + EvmData,
{
	fn execute(
		input: &[u8], //Reminder this is big-endian
		target_gas: Option<u64>,
		context: &Context,
		is_static: bool,
	) -> EvmResult<PrecompileOutput> {
		log::trace!(target: "randomness-precompile", "In randomness wrapper");

		let mut gasometer = Gasometer::new(target_gas);
		let gasometer = &mut gasometer;

		let (mut input, selector) = EvmDataReader::new_with_selector(gasometer, input)?;
		let input = &mut input;

		gasometer.check_function_modifier(context, is_static, FunctionModifier::NonPayable)?;

		match selector {
			// Getters that return values
			Action::CollectiveFlipRandom => Self::collective_flip_random(input, gasometer),
			Action::CollectiveFlipSeed => Self::collective_flip_seed(gasometer),
		}
	}
}

impl<Runtime> RandomnessWrapper<Runtime>
where
	Runtime: pallet_collective_flip::Config + pallet_evm::Config + frame_system::Config,
	Runtime::Call: Dispatchable<PostInfo = PostDispatchInfo> + GetDispatchInfo,
	<Runtime::Call as Dispatchable>::Origin: From<Option<Runtime::AccountId>>,
	Runtime::Call: From<CollectiveFlipCall<Runtime>>,
	Runtime::Hash: From<H256> + EvmData,
{
	/// Collective flip generated random output
	fn collective_flip_random(
		input: &mut EvmDataReader,
		gasometer: &mut Gasometer,
	) -> EvmResult<PrecompileOutput> {
		// Bound check
		input.expect_arguments(gasometer, 1)?;

		// Read in bytes
		let bytes = &input.read::<Bytes>(gasometer)?.0;

		let (randomness, _) = <pallet_collective_flip::Pallet<Runtime>>::random(bytes);

		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			cost: gasometer.used_gas(),
			output: EvmDataWriter::new().write(randomness).build(),
			logs: Default::default(),
		})
	}

	/// Collective flip seed material
	fn collective_flip_seed(gasometer: &mut Gasometer) -> EvmResult<PrecompileOutput> {
		// Fetch info.
		gasometer.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let random_material: Vec<Runtime::Hash> =
			<pallet_collective_flip::Pallet<Runtime>>::random_material();

		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			cost: gasometer.used_gas(),
			output: EvmDataWriter::new().write(random_material).build(),
			logs: Default::default(),
		})
	}
}
