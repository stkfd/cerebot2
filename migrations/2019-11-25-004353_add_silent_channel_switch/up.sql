alter table channels alter column command_prefix set not null;

alter table channels
	add silent boolean default false not null;
