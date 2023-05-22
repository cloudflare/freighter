drop schema if exists crates_index cascade;
create schema crates_index;
set schema 'crates_index';

create table crates
(
    id       integer primary key generated always as identity,
    name     text not null,
    registry text,
    unique nulls not distinct (name, registry)
);

create index crates_name_index on crates (name);

create table crate_versions
(
    id      integer primary key generated always as identity,
    crate   integer not null references crates (id),
    version text    not null,
    cksum   text    not null,
    yanked  bool    not null default false,
    links   text,
    unique (crate, version)
);

create index crate_versions_crate_index on crate_versions (crate);

create table features
(
    id            integer primary key generated always as identity,
    crate_version integer not null references crate_versions (id),
    name          text    not null,
    values        text[]  not null,
    unique (crate_version, name)
);

create index features_index on features (crate_version);

create type dependency_kind as enum ('normal', 'dev', 'build');

create table dependencies
(
    id               integer primary key generated always as identity,
    dependent        integer         not null references crate_versions (id),
    dependency       integer         not null references crates (id),
    req              text            not null,
    features         text[]          not null,
    optional         bool            not null,
    default_features bool            not null,
    target           text,
    kind             dependency_kind not null,
    package          text
);

create index dependencies_dependent_index on dependencies (dependent);

-- bf hash of password
create table users
(
    id            integer primary key generated always as identity,
    username      text not null unique,
    password_hash text not null
);

-- bf hash of token
create table tokens
(
    id         integer not null primary key generated always as identity,
    user_id    integer not null references users (id),
    token_hash text    not null,
    unique (user_id, token_hash)
);

create index tokens_user_index on tokens (user_id);
create index tokens_user_index on tokens (token_hash);

create table crate_owners
(
    id       integer not null primary key generated always as identity,
    user_id  integer not null references users (id),
    crate_id integer not null references crates (id),
    unique (user_id, crate_id)
);

create index crate_owners_crates_index on crate_owners (crate_id);
create index crate_owners_users_index on crate_owners (user_id);