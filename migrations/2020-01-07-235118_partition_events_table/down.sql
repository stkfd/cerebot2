create table chat_events_non_partitioned
(
    id                bigserial                   not null,
    event_type        event_type                  not null,
    twitch_message_id uuid,
    message           text,
    channel_id        integer references channels null,
    sender_user_id    integer references users,
    tags              jsonb,
    received_at       timestamp with time zone    not null,
    primary key (id)
);
insert into chat_events_non_partitioned (select * from chat_events);
drop table chat_events;
alter table chat_events_non_partitioned rename to chat_events;