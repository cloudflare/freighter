drop table if exists crates cascade;
create table crates
(
    id       integer primary key generated always as identity,
    name     text not null,
    registry text,
    unique nulls not distinct (name, registry)
);

drop table if exists crate_versions cascade;
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

create table features
(
    id            integer primary key generated always as identity,
    crate_version integer not null references crate_versions (id),
    name          text    not null,
    values        text[]  not null,
    unique (crate_version, name)
);

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

create index crates_name_index on crates (name);
create index crate_versions_crate_index on crate_versions (crate);
create index features_index on features (crate_version);
create index dependencies_dependent_index on dependencies (dependent);