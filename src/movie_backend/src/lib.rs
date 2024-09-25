#[macro_use]
extern crate serde;

use candid::{Decode, Encode};
use ic_cdk::api::time;
use ic_cdk_macros::*;
use ic_stable_structures::{memory_manager::{MemoryId, MemoryManager, VirtualMemory}, BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::{borrow::Cow, cell::RefCell};

// Type Definitions
type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct Movie {
    id: u64,
    title: String,
    duration: u32, // in minutes
    showtimes: Vec<u64>, // list of timestamps for showtimes
}

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct Booking {
    id: u64,
    movie_id: u64,
    user_id: u64,
    showtime: u64, // timestamp of the showtime
    created_at: u64,
}

// Storable implementation for Movie and Booking for stable memory storage
impl Storable for Movie {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Movie).unwrap()
    }
}

impl BoundedStorable for Movie {
    const MAX_SIZE: u32 = 1024;
    const IS_FIXED_SIZE: bool = false;
}

impl Storable for Booking {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Booking).unwrap()
    }
}

impl BoundedStorable for Booking {
    const MAX_SIZE: u32 = 1024;
    const IS_FIXED_SIZE: bool = false;
}

// Memory and Storage for Movies and Bookings
thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );

    static MOVIE_ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))), 0)
            .expect("Cannot create movie ID counter")
    );

    static BOOKING_ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1))), 0)
            .expect("Cannot create booking ID counter")
    );

    static MOVIES: RefCell<StableBTreeMap<u64, Movie, Memory>> = RefCell::new(
        StableBTreeMap::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(2))))
    );

    static BOOKINGS: RefCell<StableBTreeMap<u64, Booking, Memory>> = RefCell::new(
        StableBTreeMap::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(3))))
    );
}

// Add a new movie with showtimes
#[ic_cdk::update]
fn add_movie(title: String, duration: u32, showtimes: Vec<u64>) -> Movie {
    let id = MOVIE_ID_COUNTER.with(|counter| {
        let current_value = *counter.borrow().get();
        counter.borrow_mut().set(current_value + 1)
    }).expect("Cannot increment movie ID counter");

    let movie = Movie {
        id,
        title,
        duration,
        showtimes,
    };

    MOVIES.with(|movies| movies.borrow_mut().insert(id, movie.clone()));

    movie
}

// Get a list of all movies
#[ic_cdk::query]
fn get_movies() -> Vec<Movie> {
    MOVIES.with(|movies| {
        movies.borrow().iter().map(|(_, movie)| movie).collect()
    })
}

// Book a ticket for a movie at a specific showtime
#[ic_cdk::update]
fn book_ticket(user_id: u64, movie_id: u64, showtime: u64) -> Result<Booking, String> {
    // Check if the movie exists and showtime is valid
    let movie = MOVIES.with(|movies| movies.borrow().get(&movie_id));

    if let Some(movie) = movie {
        if !movie.showtimes.contains(&showtime) {
            return Err("Invalid showtime".to_string());
        }
    } else {
        return Err("Movie not found".to_string());
    }

    // Create a new booking
    let id = BOOKING_ID_COUNTER.with(|counter| {
        let current_value = *counter.borrow().get();
        counter.borrow_mut().set(current_value + 1)
    }).expect("Cannot increment booking ID counter");

    let booking = Booking {
        id,
        movie_id,
        user_id,
        showtime,
        created_at: time(),
    };

    BOOKINGS.with(|bookings| bookings.borrow_mut().insert(id, booking.clone()));

    Ok(booking)
}

// Get all bookings for a user
#[ic_cdk::query]
fn get_user_bookings(user_id: u64) -> Vec<Booking> {
    BOOKINGS.with(|bookings| {
        bookings.borrow().iter().filter(|(_, booking)| booking.user_id == user_id).map(|(_, booking)| booking).collect()
    })
}

// Cancel a booking
#[ic_cdk::update]
fn cancel_booking(booking_id: u64) -> Result<Booking, String> {
    let booking = BOOKINGS.with(|bookings| bookings.borrow_mut().remove(&booking_id));

    match booking {
        Some(booking) => Ok(booking),
        None => Err("Booking not found".to_string()),
    }
}

// Generate the candid interface
ic_cdk::export_candid!();
