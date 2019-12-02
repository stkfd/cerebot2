alter table command_attributes drop column handler_name;

alter table command_attributes add column name text not null default '';

drop table command_aliases;
