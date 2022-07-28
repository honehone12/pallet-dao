#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/v3/runtime/frame>
pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::{
		pallet_prelude::{*, ValueQuery},
		transactional,
		dispatch::DispatchResult,
		sp_runtime::{ traits::Hash, SaturatedConversion, AccountId32 },
		traits::{ Currency, ConstU32, Randomness, ExistenceRequirement },
		Twox64Concat, BoundedVec, Twox256,
	};
	use frame_system::pallet_prelude::*;
	use scale_info::TypeInfo;
	use sp_io::hashing::blake2_128;

	#[cfg(feature = "std")]
	use frame_support::serde::{ Serialize, Deserialize };

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type Currency: Currency<Self::AccountId>;
		type Randomness: Randomness<Self::Hash, Self::BlockNumber>;
		#[pallet::constant]
		type MaxNumMembershipOwned: Get<u32>;
		#[pallet::constant]
		type DefaultNumMembership: Get<u64>;
		#[pallet::constant]
		type DefaultMembershipPrice: Get<u64>;
		#[pallet::constant]
		type DefaultContinueBlocks: Get<u64>;
	}
	
	type AccountOf<T> = <T as frame_system::Config>::AccountId;
	type BalanceOf<T>
		= <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
	type MaxNumHoldTicket = ConstU32<32>;
	type MaxByteShortString = ConstU32<64>;
	type MaxByteMiddleString = ConstU32<128>;

	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	#[codec(mel_bound())]
	pub struct DaoMembership<T: Config> {
		dna: Option<[u8; 16]>,
		owner: Option<AccountOf<T>>,
		price: Option<BalanceOf<T>>,
		name: Option<BoundedVec<u8, MaxByteShortString>>,
	}

	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	#[codec(mel_bound())]
	pub struct Proposal<T: Config> {
		proposer: AccountOf<T>,
		subject: BoundedVec<u8, MaxByteShortString>,
		description: BoundedVec<u8, MaxByteMiddleString>,
		how: BoundedVec<u8, MaxByteMiddleString>,
		is_open: bool,
		end_block_number: u64
	}

	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen, Default)]
	#[scale_info(skip_type_params(T))]
	#[codec(mel_bound())]
	pub struct VoteStats {
		yes: u64,
		no: u64
	}

	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	#[codec(mel_bound())]
	pub struct Ticket<T: Config>(T::Hash);

	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
	pub enum Vote {
		Yes,
		No
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	// The pallet's runtime storage items.
	// https://docs.substrate.io/v3/runtime/storage
	// Learn more about declaring storage items:
	// https://docs.substrate.io/v3/runtime/storage#declaring-storage-items
	#[pallet::storage]
	#[pallet::getter(fn count_for_pooled)]
	pub(super) type CountForPooled<T: Config>
		= StorageValue<_, u64, ValueQuery>;
	
	#[pallet::storage]
	#[pallet::getter(fn memberships_pooled)]
	pub type MembershipsPooled<T: Config> = StorageMap<
		_,
		Twox256,
		u64,
		DaoMembership<T>
	>;

	#[pallet::storage]
	#[pallet::getter(fn count_for_owned)]
	pub(super) type CountForOwned<T: Config> 
		= StorageValue<_, u64, ValueQuery>;
	
	#[pallet::storage]
	#[pallet::getter(fn memberships)]
	pub(super) type Memberships<T: Config> = StorageMap<
		_,
		Twox64Concat,
		T::Hash,
		DaoMembership<T>
	>;

	#[pallet::storage]
	#[pallet::getter(fn memberships_owned)]
	pub(super) type MembershipsOwned<T: Config> = StorageMap<
		_,
		Twox64Concat,
		T::AccountId,
		BoundedVec<T::Hash, T::MaxNumMembershipOwned>,
		ValueQuery
	>;

	#[pallet::storage]
	#[pallet::getter(fn proposals)]
	pub(super) type Proposals<T: Config> = StorageMap<
		_,
		Twox64Concat,
		T::Hash,
		Proposal<T>,
	>;

	#[pallet::storage]
	#[pallet::getter(fn votes)]
	pub(super) type Votes<T: Config> = StorageMap<
		_,
		Twox64Concat,
		T::Hash,
		VoteStats,
		ValueQuery
	>;
 
	#[pallet::storage]
	#[pallet::getter(fn tickets)]
	pub(super) type Tickets<T: Config> = StorageMap<
		_,
		Twox64Concat,
		T::AccountId,
		BoundedVec<Ticket<T>, MaxNumHoldTicket>,
		ValueQuery
	>;

	// Pallets use events to inform users when important changes are made.
	// https://docs.substrate.io/v3/runtime/events-and-errors
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Event documentation should end with an array that provides descriptive names for event
		/// parameters. [something, who]
		
		//membership evnets
		Bought(T::AccountId, T::Hash, BalanceOf<T>),
		PriceSet(T::AccountId, T::Hash, Option<BalanceOf<T>>),
		NameSet(T::AccountId, T::Hash, Option<BoundedVec<u8, ConstU32<64>>>),
		Exchanged(T::AccountId, T::AccountId, T::Hash, BalanceOf<T>),
		Transfered(T::AccountId, T::AccountId, T::Hash),

		//dao functionality events
		Proposed(T::AccountId, T::Hash),
		TicketProvided(T::Hash, u64, u64),
		Voted(T::Hash, Vote),
		VoteClosed(T::Hash)
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// Error names should be descriptive.
		/// Errors should have helpful documentation associated with them.
		
		// system error
		AccountIdNotSet,
		OwnerNotSet,
		CountsOverflow,
		CountsUnderflow,
		RuntimeAccountDecodeError,
		StringConversionError,

		//user error
		MembershipNotExists,
		NoMorePooled,
		ExceedMaxMembershipOwned,
		BidPriceNotEnough,
		NotEnoughBalance,
		NotForSale,
		NotMembershipOwner,
		ExchangeWithSelf,
		TransferToSelf,
		TicketNotFound,
		ProposalNotExists,
		VoteAlreadyClosed,
		NotProposer,
	}

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		default_price: BalanceOf<T>
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> GenesisConfig<T> {
			GenesisConfig {
				default_price: T::DefaultMembershipPrice::get()
					.saturated_into()
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			let n = T::DefaultNumMembership::get(); 
			for i in 0u64..n
			{
				let val = DaoMembership {
					dna: None,
					owner: None,
					price: Some(self.default_price.clone()),
					name: None,
				};
				<MembershipsPooled<T>>::insert(i, val);
				log::info!(
					"[info!!] generated membership and pooled: [{:?}]",
					i
				);
			}
			<CountForPooled<T>>::put(n);
			<CountForOwned<T>>::put(0);
		}
	}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		// storage and emits an event. This function must be dispatched by a signed extrinsic.
		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(1,1))]
		pub fn buy_membership(
			origin: OriginFor<T>,
			bid_price: BalanceOf<T>,
		) -> DispatchResult {
			// Check that the extrinsic was signed and get the signer.
			// This function will return an error if the extrinsic is not signed.
			// https://docs.substrate.io/v3/runtime/origins
			let caller = ensure_signed(origin)?;

			// check still membership pool remains
			let current_pooled_count = Self::count_for_pooled();
			ensure!(
				current_pooled_count > 0,
				<Error<T>>::NoMorePooled
			);

			// ensure membership is for sale
			let current_pooled_idx = current_pooled_count - 1;
			let mut bought_membership = Self::memberships_pooled(current_pooled_idx)
				.ok_or(<Error<T>>::MembershipNotExists)?;
			if let Some(membership_price) = bought_membership.price {
				ensure!(
					bid_price >= membership_price,
					<Error<T>>::BidPriceNotEnough
				);
			} else {
				return Err(<Error<T>>::NotForSale)?
			}

			// ensure buyer has enough balance
			ensure!(
				T::Currency::free_balance(&caller) >= bid_price,
				<Error<T>>::NotEnoughBalance
			);

			// ensure buyer have room for membership
			ensure!(
				Self::memberships_owned(&caller).len() < T::MaxNumMembershipOwned::get() as usize,
				<Error<T>>::ExceedMaxMembershipOwned
			);

			// remove from pool
			<MembershipsPooled<T>>::remove(current_pooled_idx);
			
			// set struct members
			bought_membership.dna = Some(Self::gen_dna());
			bought_membership.owner = Some(caller.clone());
			bought_membership.price = None;

			// calc membership hash = id
			let membership_id = T::Hashing::hash_of(&bought_membership);
			// set counts
			let new_cnt_owned = Self::count_for_owned()
				.checked_add(1)
				.ok_or(<Error<T>>::CountsOverflow)?;
			<CountForOwned<T>>::put(new_cnt_owned);
			let new_cnt_pooled = Self::count_for_pooled()
				.checked_sub(1)
				.ok_or(<Error<T>>::CountsUnderflow)?;
			// set(insert) storage
			<CountForPooled<T>>::put(new_cnt_pooled);
			<Memberships<T>>::insert(membership_id, bought_membership);
			<MembershipsOwned<T>>::try_mutate(
				&caller,
				|owned| owned.try_push(membership_id)
			).map_err(|_| <Error<T>>::ExceedMaxMembershipOwned)?;

			log::info!(
				"[info!!] a membership is bought by: {:?} id: {:?} price: {:?}.",
				caller,
				membership_id,
				bid_price
			);

			// transfer tokens
			//now alice is admin for convinience
			let account = hex_literal::hex![
				"d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"
			].into();
			let admin
				= T::AccountId::decode(&mut AccountId32::as_ref(&account))
				.map_err(|_| <Error<T>>::RuntimeAccountDecodeError)?;
			T::Currency::transfer(
				&caller,
				&admin,
				bid_price,
				ExistenceRequirement::KeepAlive
			)?;

			// fire event
			Self::deposit_event(
				Event::Bought(caller, membership_id, bid_price)
			);
			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(1,1))]
		pub fn set_price(
			origin: OriginFor<T>,
			membership_id: T::Hash,
			new_price: Option<BalanceOf<T>>
		) -> DispatchResult {
			let caller = ensure_signed(origin)?;

			// ensure membership owner
			ensure!(
				Self::is_membership_owner(&membership_id, &caller)?,
				<Error<T>>::NotMembershipOwner
			);

			let mut target_membership = Self::memberships(&membership_id)
				.ok_or(<Error<T>>::MembershipNotExists)?;

			// set new price and put to storage
			target_membership.price = new_price.clone();
			<Memberships<T>>::insert(&membership_id, target_membership);
			log::info!(
				"[info!!] a membership is set. ID: {:?} Price: {:?}.",
				membership_id,
				new_price
			);

			//fire event
			Self::deposit_event(
				Event::PriceSet(caller, membership_id, new_price)
			);
			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(1,1))]
		pub fn set_name(
			origin: OriginFor<T>,
			membership_id: T::Hash,
			new_name: Option<BoundedVec<u8, ConstU32<64>>>
		) -> DispatchResult {
			let caller = ensure_signed(origin)?;

			// ensure membership owner
			ensure!(
				Self::is_membership_owner(&membership_id, &caller)?,
				<Error<T>>::NotMembershipOwner
			);

			let mut target_membership = Self::memberships(&membership_id)
				.ok_or(<Error<T>>::MembershipNotExists)?;

			// set new name
			target_membership.name = new_name.clone();

			//put new to storage
			<Memberships<T>>::insert(&membership_id, target_membership);
			log::info!(
				"[info!!] a membership is set. ID: {:?} Name: {:?}.",
				membership_id,
				new_name
			);

			// fire event
			Self::deposit_event(
				Event::NameSet(caller, membership_id, new_name)
			);
			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(1,1))]
		pub fn exchange_membership(
			origin: OriginFor<T>,
			membership_id: T::Hash,
			bid_price: BalanceOf<T>
		) -> DispatchResult {
			// caller is buyer
			let buyer = ensure_signed(origin)?;

			let target_membership = Self::memberships(&membership_id)
				.ok_or(<Error<T>>::MembershipNotExists)?;

			// ensure buyer is not owner
			if let Some(seller) = target_membership.owner {
				ensure!(
					buyer != seller,
					<Error<T>>::ExchangeWithSelf
				);

				// ensure buyer prepare enough balance
				if let Some(p) = target_membership.price {
					ensure!(
						bid_price >= p,
						<Error<T>>::BidPriceNotEnough
					);

				} else {
					return Err(<Error<T>>::NotForSale)?
				}
				ensure!(
					T::Currency::free_balance(&buyer) >= bid_price,
					<Error<T>>::NotEnoughBalance
				);
				
				// ensure buyer has room for membership
				let owned_by_buyer
					= Self::memberships_owned(&buyer);
				ensure!(
					owned_by_buyer.len() < T::MaxNumMembershipOwned::get() as usize,
					<Error<T>>::ExceedMaxMembershipOwned
				);

				// transfer currency
				T::Currency::transfer(
					&buyer,
					&seller,
					bid_price,
					ExistenceRequirement::KeepAlive
				)?;
				log::info!(
					"[info!!] transfered currency FROM: {:?} TO: {:?} Amount: {:?}.",
					buyer,
					seller,
					bid_price
				);

				//transfer membership
				Self::transfer_membership_impl(
					&membership_id,
					&buyer,
				)?;
				log::info!(
					"[info!!] transfered membership ID: {:?} FROM: {:?} TO: {:?}.",
					membership_id,
					seller,
					buyer
				);

				Self::deposit_event(
					Event::Exchanged(seller, buyer, membership_id, bid_price)
				);
			} else {
				return Err(<Error<T>>::OwnerNotSet)?
			}
			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(10))]
		pub fn propose(
			origin: OriginFor<T>,
			subject: BoundedVec<u8, ConstU32<64>>,
			description: BoundedVec<u8, ConstU32<128>>,
			how: BoundedVec<u8, ConstU32<128>>,
			continue_blocks: u64
		) -> DispatchResult {
			let caller = ensure_signed(origin)?;
			
			// eusure account has membership
			ensure!(
				Self::memberships_owned(&caller).len() > 0,
				<Error<T>>::NotMembershipOwner
			);

			// set how long vote available
			let mut continue_num_blocks = continue_blocks;
			if continue_blocks == 0u64 {
				continue_num_blocks = T::DefaultContinueBlocks::get();
			}

			let current_block_num: u64 
				= <frame_system::Pallet<T>>::block_number()
				.saturated_into();

			// put to storage
			let proposal: Proposal<T> = Proposal {
				proposer: caller.clone(),
				subject,
				description,
				how,
				is_open: true,
				end_block_number: current_block_num + continue_num_blocks
			};
			let proposal_id = T::Hashing::hash_of(&proposal);
			<Proposals<T>>::insert(&proposal_id, &proposal);
			log::info!(
				"[info!!] Account: {:?} proposed ID: {:?} last until BlockNumber: {:?}.",
				caller,
				proposal_id,
				current_block_num + continue_num_blocks
			);
			// fire event
			Self::deposit_event(
				Event::Proposed(caller, proposal_id)
			);
			
			//suply tickets
			let (success, fail) = Self::supply_tickets(&proposal_id);
			log::info!(
				"[info!!] [{:?}] tickets are successfully suplied, [{:?}] are failed.",
				success,
				fail	
			);
			// fire (tickets) event
			Self::deposit_event(
				Event::TicketProvided(proposal_id, success, fail)
			);
			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(1,1))]
		pub fn close_vote(origin: OriginFor<T>, proposal_id: T::Hash
		) -> DispatchResult {
			let caller = ensure_signed(origin)?;

			// check membership owner
			ensure!(
				Self::memberships_owned(&caller).len() > 0,
				<Error<T>>::NotMembershipOwner
			);

			// check the proposal exists and is open now
			let mut target_proposal = Self::proposals(&proposal_id)
				.ok_or(<Error<T>>::ProposalNotExists)?;
			ensure!(
				target_proposal.is_open,
				<Error<T>>::VoteAlreadyClosed
			);
			// ensure caller is proposer
			ensure!(
				target_proposal.proposer == caller,
				<Error<T>>::NotProposer
			);

			// close
			target_proposal.is_open = false;
			<Proposals<T>>::insert(&proposal_id, &target_proposal);
			log::info!(
				"[info!!] proposal ID: {:?} is now closed.",
				proposal_id
			);

			// fire event
			Self::deposit_event(
				Event::VoteClosed(proposal_id)
			);
			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(1,1))]
		pub fn vote(
			origin: OriginFor<T>,
			proposal_id: T::Hash,
			vote: Vote
		) -> DispatchResult {
			let caller = ensure_signed(origin)?;

			// check membership owner
			ensure!(
				Self::memberships_owned(&caller).len() > 0,
				<Error<T>>::NotMembershipOwner
			);

			// check the proposal exists and is open
			// close if current block num is over than end block num
			let mut target_proposal = Self::proposals(&proposal_id)
				.ok_or(<Error<T>>::ProposalNotExists)?;
			let current_blocks: u64 = <frame_system::Pallet<T>>::block_number()
				.saturated_into();
			// WARN: if noone votes, will not close foreever.
			// shold have better way
			if current_blocks >= target_proposal.end_block_number {
				target_proposal.is_open = false;
				log::info!(
					"vote ID: {:?} is closed at end block number AT: programed={:?} actual={:?}.",
					proposal_id,
					target_proposal.end_block_number,
					current_blocks
				);
			}
			ensure!(
				target_proposal.is_open,
				<Error<T>>::VoteAlreadyClosed
			);

			// consume ticket
			Self::consume_ticket(&caller, &proposal_id)?;

			// put vote to storage
			let mut stats = Self::votes(&proposal_id);
			match vote {
				Vote::Yes => {
					stats.yes = stats.yes.checked_add(1)
						.ok_or(<Error<T>>::CountsOverflow)?;
				},
				Vote::No => {
					stats.no = stats.no.checked_sub(1)
						.ok_or(<Error<T>>::CountsUnderflow)?;
				}
			}
			<Votes<T>>::insert(&proposal_id, stats);
			log::info!(
				"[info!!] voted ID: {:?}, {:?}",
				proposal_id,
				vote
			);
			Self::deposit_event(
				Event::Voted(proposal_id, vote)
			);
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		fn gen_dna() -> [u8; 16] {
			let payload = (
				T::Randomness::random(&b"deeenee"[..]).0,
				<frame_system::Pallet<T>>::extrinsic_index().unwrap_or_default(),
				<frame_system::Pallet<T>>::block_number(),
			);
			payload.using_encoded(blake2_128)
		}

		fn is_membership_owner(
			membership_id: &T::Hash,
			account: &T::AccountId,
		) -> Result<bool, Error<T>> {
			match Self::memberships(membership_id) {
				Some(ms) => {
					match ms.owner {
						Some(a) => Ok(a == *account), 
						None => Err(<Error<T>>::AccountIdNotSet)
					}
				},
				None => Err(<Error<T>>::MembershipNotExists)
			}
		}

		#[transactional]
		fn transfer_membership_impl(
			membership_id: &T::Hash,
			to: &T::AccountId
		) -> Result<(), Error<T>> {
			let mut target_membership = Self::memberships(&membership_id)
				.ok_or(<Error<T>>::MembershipNotExists)?;
			if let Some(prev_owner) = target_membership.owner.clone() {
				// long, but just find and remove from ownerships owned storage
				<MembershipsOwned<T>>::try_mutate(
					prev_owner,
					|owned| {
						if let Some(prev_idx) = owned.iter()
							.position(|&id| id == *membership_id
						) {
							owned.swap_remove(prev_idx);
							return Ok(())
						}
						Err(())
					}
				).map_err(|_| <Error<T>>::MembershipNotExists)?;
			} else {
				return Err(<Error<T>>::AccountIdNotSet)
			}
			
			//put new to storage
			target_membership.owner = Some(to.clone());
			target_membership.price = None;
			target_membership.name = None;
			<Memberships<T>>::insert(membership_id, target_membership);
			<MembershipsOwned<T>>::try_mutate(
				to,
				|owned| owned.try_push(*membership_id)
			).map_err(|_| <Error<T>>::ExceedMaxMembershipOwned)?;
			Ok(())
		}

		fn supply_tickets(
			proposal_id: &T::Hash,
		) -> (u64, u64) {
			let mut done = 0u64;
			let mut skipped = 0u64;
			for k in <MembershipsOwned<T>>::iter_keys() {
				let mut bv = Self::tickets(&k);
				let res = bv.try_push(Ticket(proposal_id.clone()));
				match res {
					Ok(()) => {
						done += 1u64;
						// log::info!(
						// 	"[info!!] Tiket: {:?} is suplied to Account: {:?}.",
						// 	proposal_id,
						// 	k
						// );
					},
					Err(()) => {
						skipped += 1u64;
						// now we just skip, should be changed if nicer way was found.
						// log::info!(
						// 	"[info!!] Account: {:?} 's storage is full. skipped.",
						// 	k
						// );
					}
				}
				<Tickets<T>>::insert(k, bv);
			}
			(done, skipped)
		}

		fn consume_ticket(
			account_id: &T::AccountId,
			proposal_id: &T::Hash,
		) -> Result<(), Error<T>> {
			let mut bv = Self::tickets(account_id);
			if bv.len() < 1 {
				return Err(<Error<T>>::TicketNotFound)
			}

			if let Some(idx) = bv.iter()
				.position(|id| id.0 == *proposal_id
			) {
				bv.swap_remove(idx);
				<Tickets<T>>::insert(account_id, bv);
				log::info!(
					"[info!!] Ticket: {:?} for AccountId: {:?} cosumed.",
					proposal_id,
					account_id
				);
				Ok(())
			} else {
				Err(<Error<T>>::TicketNotFound)
			}
		}
	}
}
