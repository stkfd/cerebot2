create table command_attributes
(
	id serial not null primary key,
    name text not null,
    description text,
    enabled boolean not null default true,
    default_active boolean not null,
    cooldown integer constraint command_cooldown_positive check (cooldown >= 0)
);

create table command_permissions
(
    command_id integer not null references command_attributes(id),
    permission_id integer not null references permissions(id),
    primary key (command_id, permission_id)
);

create table channel_command_config
(
    channel_id integer not null references channels(id),
    command_id integer not null references command_attributes(id),
    active boolean,
    cooldown integer constraint channel_command_cooldown_positive check (cooldown >= 0),
    primary key (channel_id, command_id)
);
