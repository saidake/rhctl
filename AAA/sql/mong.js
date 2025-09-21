db.createCollection("test");
db.test.insertMany(
    [{
        "entity_no":"233456",
        "case_name":"Craig Test",
        "create_time":new Date(),
        "sort":NumberInt(10),
        "state":null
    },
    {
        "entity_no":"123456",
        "case_name":"Alice Test",
        "create_time":new Date(),
        "sort":NumberInt(20),
        "state":"deleted"
    }]
    );