create table implied_permissions
(
	permission_id int references permissions,
	implied_by_id int references permissions,
	primary key (permission_id, implied_by_id)
);

alter table command_attributes
    add whisper_enabled boolean default true not null;
