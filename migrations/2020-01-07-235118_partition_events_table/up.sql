create table chat_events_partitioned
(
    id                bigserial                   not null,
    event_type        event_type                  not null,
    twitch_message_id uuid,
    message           text,
    channel_id        integer references channels null,
    sender_user_id    integer references users,
    tags              jsonb,
    received_at       timestamp with time zone    not null,
    unique (channel_id, received_at, id)
) partition by list (channel_id);

create table chat_events_ch0 partition of chat_events_partitioned for values in (null) partition by range (received_at);
create table chat_events_ch0_default partition of chat_events_ch0 for values from (minvalue) to (maxvalue);

do
$$
    declare
        cid               int;
        partition_name    text;
        subpartition_name text;
    begin
        for cid in select distinct on (channel_id) channel_id from chat_events where channel_id is not null
            loop
                partition_name := format('chat_events_ch%s', cid);
                subpartition_name := format('chat_events_ch%s_default', cid, extract(epoch from now()));
                raise notice 'Creating partition table for channel % (%, %)', cid, partition_name, subpartition_name;
                execute format(
                        'create table %I partition of chat_events_partitioned for values in (%s) partition by range(received_at)',
                        partition_name, cid);
                execute format('create table %I partition of %I for values from (MINVALUE) to (MAXVALUE)',
                               subpartition_name, partition_name);
            end loop;
    end
$$;
insert into chat_events_partitioned (select * from chat_events);
drop table chat_events;
alter table chat_events_partitioned rename to chat_events;