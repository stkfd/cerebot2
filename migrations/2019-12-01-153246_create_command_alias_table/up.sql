create table command_aliases
(
    name text primary key,
    command_id integer references command_attributes(id) not null
);

alter table command_attributes drop column name;

alter table command_attributes add column handler_name text not null;
