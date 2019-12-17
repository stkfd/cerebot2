table! {
    channel_command_config (channel_id, command_id) {
        channel_id -> Int4,
        command_id -> Int4,
        active -> Nullable<Bool>,
        cooldown -> Nullable<Int4>,
    }
}

table! {
    channels (id) {
        id -> Int4,
        twitch_room_id -> Nullable<Int4>,
        name -> Varchar,
        join_on_start -> Bool,
        command_prefix -> Nullable<Varchar>,
        updated_at -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
        silent -> Bool,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::db::chat_event::ChatEventTypeMapping;

    chat_events (id) {
        id -> Int8,
        event_type -> ChatEventTypeMapping,
        twitch_message_id -> Nullable<Uuid>,
        message -> Nullable<Text>,
        channel_id -> Nullable<Int4>,
        sender_user_id -> Nullable<Int4>,
        tags -> Nullable<Jsonb>,
        received_at -> Timestamptz,
    }
}

table! {
    command_aliases (name) {
        name -> Text,
        command_id -> Int4,
    }
}

table! {
    command_attributes (id) {
        id -> Int4,
        description -> Nullable<Text>,
        enabled -> Bool,
        default_active -> Bool,
        cooldown -> Nullable<Int4>,
        whisper_enabled -> Bool,
        handler_name -> Text,
        template -> Nullable<Text>,
    }
}

table! {
    command_permissions (command_id, permission_id) {
        command_id -> Int4,
        permission_id -> Int4,
    }
}

table! {
    implied_permissions (permission_id, implied_by_id) {
        permission_id -> Int4,
        implied_by_id -> Int4,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::db::permissions::PermissionStateMapping;

    permissions (id) {
        id -> Int4,
        name -> Text,
        description -> Nullable<Text>,
        default_state -> PermissionStateMapping,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::db::permissions::PermissionStateMapping;

    user_permissions (permission_id, user_id) {
        user_id -> Int4,
        permission_id -> Int4,
        user_permission_state -> PermissionStateMapping,
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

joinable!(channel_command_config -> channels (channel_id));
joinable!(channel_command_config -> command_attributes (command_id));
joinable!(chat_events -> channels (channel_id));
joinable!(chat_events -> users (sender_user_id));
joinable!(command_aliases -> command_attributes (command_id));
joinable!(command_permissions -> command_attributes (command_id));
joinable!(command_permissions -> permissions (permission_id));
joinable!(user_permissions -> permissions (permission_id));
joinable!(user_permissions -> users (user_id));

allow_tables_to_appear_in_same_query!(
    channel_command_config,
    channels,
    chat_events,
    command_aliases,
    command_attributes,
    command_permissions,
    implied_permissions,
    permissions,
    user_permissions,
    users,
);
