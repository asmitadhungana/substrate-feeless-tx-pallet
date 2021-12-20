#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::{
		dispatch::{DispatchResult, Dispatchable, DispatchResultWithPostInfo},
		pallet_prelude::*,
		Parameter,
		weights::{Pays, GetDispatchInfo},
		traits::Get,
		sp_runtime::traits::BlockNumberProvider,
		
	};
	use frame_system::pallet_prelude::*;
	use scale_info::prelude::boxed::Box;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		// The call type from the runtime which has all the calls available in your runtime.
		type Call: Parameter + GetDispatchInfo + Dispatchable<Origin=Self::Origin>;	

		// The maximum amount of calls an account can make
		#[pallet::constant]
		type MaxCalls: Get<u32>; // Having MaxCalls as a configuration trait instead of a StorageItem saves one storage read per block for every time the make_feeless extrinsic is called
									// Configuration is totally free since it's like a hard-coded variable, but it cannot be altered without upgrading the runtime if need arises, while if
									// if MaxCalls had been a storage item, governance could change it by calling an extrinsic w/o having to upgrade the runtime

		#[pallet::constant]
		type SessionLength: Get<<Self as frame_system::Config>::BlockNumber>;
	}

	#[pallet::error]
	pub enum Error<T> {
		// An account cannot make more Calls than 'MaxCalls'.
		ExceedMaxCalls,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		// TODO
		ExtrinsicResult{
			tx_sender: T::AccountId,
			// T::AccountId, 
			feeless_result: DispatchResult
		}, // Dispatch Result so that it can give the error msg in case error arises
	}

	#[pallet::storage]
	#[pallet::getter(fn tracker)]
	// Track how many calls each user has done for the latest session
	pub(super) type Tracker <T: Config> = StorageMap<_, Twox64Concat, T::AccountId, (T::BlockNumber, u32), ValueQuery>;


	#[pallet::call]
	impl<T: Config> Pallet<T> {

		#[pallet::weight({
			let dispatch_info = call.get_dispatch_info();
			(dispatch_info.weight.saturating_add(10_000), dispatch_info.class, Pays::Yes)
		})]
		pub fn make_feeless(
			origin: OriginFor<T>,
			call: Box<<T as Config>::Call>,
		) -> DispatchResultWithPostInfo {

			let sender = ensure_signed(origin.clone())?;

			// Get the relevant storage data.
			let max_calls = T::MaxCalls::get();
			let (last_user_session, mut user_calls) = Tracker::<T>::get(&sender);
			let current_blocknumber: T::BlockNumber = frame_system::Pallet::<T>::current_block_number();
			let session_length = T::SessionLength::get();

			// Calculate the current session.
			let current_session = current_blocknumber / session_length;	// If the session length is 1000, and I'm in blocknumber 2400, current session = 2 | if I'm in blocknumber 4000, current session = 4 and likewise

			// If this is a new session for the user, reset their count.
			if last_user_session < current_session {
				user_calls = 0;
			}

			// Check that the user has an available free call
			if user_calls < max_calls {
				// Update the tracker count.
				Tracker::<T>::insert(
					&sender,
					(
						current_session,
						user_calls.saturating_add(1),
					)
				);

				// Dispatch the call
				let result = call.dispatch(origin);

				Self::deposit_event(Event::ExtrinsicResult { tx_sender: sender, feeless_result: result.map(|_| ()).map_err(|e| e.error) });

				// Self::deposit_event(
				// 	Event::ExtrinsicResult(
				// 		sender, 
				// 		result.map(|_| ()).map_err(|e| e.error),
				// 	)
				// );

				// Make the tx feeless!
				return Ok(Pays::No.into()) // 
			} else {
				// They do not have enough feeless txs, so we charge them
				// for the reads.
				//
				// Note: This could be moved into a signed extension check to
				// avoid charging them any fees at all in any situation.

				// Function returns a calculation corresponding to 3 DB reads: weights consumed if the user couldn't get a free txn [i.e. if s/he fails the if case for user_calls < max_calls]
				let check_logic_weight = T::DbWeight::get().reads(3);
				// Return the reduced weight: Charge the user with the fee equivalent to weight of 3 storage reads
				return Ok(Some(check_logic_weight).into())
			}
		}
	}
}
