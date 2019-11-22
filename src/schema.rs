table! {
    channels (id) {
        id -> Int4,
        twitch_room_id -> Nullable<Int4>,
        name -> Varchar,
        join_on_start -> Bool,
        command_prefix -> Nullable<Varchar>,
        updated_at -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::db::ChatEventTypeMapping;

    chat_events (id) {
        id -> Int8,
        event_type -> ChatEventTypeMapping,
        twitch_message_id -> Nullable<Uuid>,
        message -> Nullable<Text>,
        channel_id -> Nullable<Int4>,
        sender_user_id -> Nullable<Int4>,
        tags -> Nullable<Jsonb>,
        sent_at -> Nullable<Timestamptz>,
        received_at -> Timestamptz,
    }
}

table! {
    users (id) {
        id -> Int4,
        twitch_user_id -> Int4,
        name -> Varchar,
        display_name -> Nullable<Varchar>,
        previous_names -> Nullable<Array<Text>>,
        previous_display_names -> Nullable<Array<Text>>,
        updated_at -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
    }
}

joinable!(chat_events -> channels (channel_id));
joinable!(chat_events -> users (sender_user_id));

allow_tables_to_appear_in_same_query!(channels, chat_events, users,);
