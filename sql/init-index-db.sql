drop table if exists crates cascade;
create table crates
(
    id            integer primary key generated always as identity,
    name          text not null,
    registry      text not null default '',
    description   text,
    documentation text,
    homepage      text,
    repository    text,
    unique (name, registry)
);

drop table if exists keywords cascade;
create table keywords
(
    id   integer primary key generated always as identity,
    name text not null unique
);

drop table if exists categories cascade;
create table categories
(
    id   integer primary key generated always as identity,
    name text not null unique
);

drop table if exists crate_keywords cascade;
create table crate_keywords
(
    id      integer primary key generated always as identity,
    crate   integer not null references crates (id),
    keyword integer not null references keywords (id)
);

drop table if exists crate_categories cascade;
create table crate_categories
(
    id       integer primary key generated always as identity,
    crate    integer not null references crates (id),
    category integer not null references categories (id)
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

drop table if exists features cascade;
create table features
(
    id            integer primary key generated always as identity,
    crate_version integer not null references crate_versions (id),
    name          text    not null,
    values        text[]  not null,
    unique (crate_version, name)
);

drop type if exists dependency_kind cascade;
create type dependency_kind as enum ('normal', 'dev', 'build');

drop table if exists dependencies cascade;
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

create index crate_keyword_crate on crate_keywords (crate);
create index crate_keyword_keyword on crate_keywords (keyword);
create index crate_categories_crate on crate_keywords (crate);
create index crate_categories_category on crate_categories (category);
create index crates_name_index on crates (name);
create index crate_versions_crate_index on crate_versions (crate);
create index features_index on features (crate_version);
create index dependencies_dependent_index on dependencies (dependent);
