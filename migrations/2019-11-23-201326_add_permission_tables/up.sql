create type permission_state as enum (
    'allow',
    'deny'
);

create table permissions
(
    id serial not null
        constraint permissions_pk
            primary key,
    name text unique not null,
    description text,
    default_state permission_state not null
);

create table user_permissions
(
    user_id integer references users(id) on delete cascade,
    permission_id integer references permissions(id) on delete cascade,
    user_permission_state permission_state not null,
    primary key (permission_id, user_id)
);
