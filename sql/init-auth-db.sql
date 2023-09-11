create extension if not exists pgcrypto;

drop table if exists freighter_users cascade;

create table freighter_users
(
    id            integer primary key generated always as identity,
    username      text not null unique
);

drop table if exists freighter_tokens cascade;

-- bf hash of token
create table freighter_tokens
(
    id         integer not null primary key generated always as identity,
    user_id    integer not null references freighter_users (id),
    token_hash text    not null unique
);

drop table if exists freighter_crate_owners cascade;

create table freighter_crate_owners
(
    id      integer not null primary key generated always as identity,
    user_id integer not null references freighter_users (id),
    crate   text    not null,
    unique (user_id, crate)
);

create index freighter_tokens_user_index on freighter_tokens (user_id);
create index freighter_tokens_hash_index on freighter_tokens (token_hash);
create index freighter_crate_owners_crates_index on freighter_crate_owners (crate);
create index freighter_crate_owners_users_index on freighter_crate_owners (user_id);
