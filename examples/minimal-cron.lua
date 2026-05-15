sources = {
	["cron"] = {
		["type"] = "cron",
	},
}

triggers = {
	{
		["source"] = "cron",
		["emit"] = "2-trigger",
		["arguments"] = "*/2 * * * * *",
	},
	{
		["source"] = "cron",
		["emit"] = "3-trigger",
		["arguments"] = "*/3 * * * * *",
	},
}

handlers = {
	["debug"] = {
		["type"] = "debug",
	},
}

actions = {
	{
		["handler"] = "debug",
		["event"] = "2-trigger",
		["emit"] = "2-debug",
	},
	{
		["handler"] = "debug",
		["event"] = "3-trigger",
		["emit"] = "3-debug",
	},
}
