create table channels
(
	id serial primary key,
	twitch_room_id integer,
	name varchar(200) not null,
	join_on_start boolean default false not null,
	command_prefix varchar(20),
	updated_at timestamptz,
    created_at timestamptz default now()
);

create table users
(
    id serial primary key,
    twitch_user_id integer not null
        constraint users_twitch_user_id_key
            unique,
    name varchar(200) not null,
    display_name varchar(200),
    previous_names text[],
    previous_display_names text[],
    updated_at timestamptz,
    created_at timestamptz default now()
);

create unique index users_twitch_id_index
    on users (twitch_user_id);

create unique index users_name_index
    on users (name);

create type event_type as enum ('privmsg', 'whisper', 'notice', 'usernotice', 'host', 'clearchat', 'clearmsg', 'roomstate', 'connect');

create table chat_events
(
    id bigserial primary key,
    event_type event_type not null,
    twitch_message_id uuid,
    message text,
    channel_id integer,
    sender_user_id integer,
    tags jsonb,
    received_at timestamptz not null
);

alter table chat_events
    add constraint chat_events_user_id_fk
        foreign key (sender_user_id) references users;

alter table chat_events
    add constraint chat_events_channel_id_fk
        foreign key (channel_id) references channels;
