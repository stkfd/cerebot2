alter table channels alter column command_prefix drop not null;

alter table channels drop column silent;
