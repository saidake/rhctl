------------------------------------------------------------------------------ TEST_PERSON
create table TEST_PERSON(
    PER_ID NUMBER PRIMARY KEY,
    AGE NUMBER,
    NAME VARCHAR2(30),
    GENDER NUMBER(1)
);
INSERT INTO TEST_PERSON VALUES(1,23,'david',1);
INSERT INTO TEST_PERSON VALUES(2,22,'rose',0);
INSERT INTO TEST_PERSON VALUES(3,25,'jack',1);

------------------------------------------------------------------------------ TEST_STUDENT
create table TEST_STUDENT(
    STU_ID NUMBER PRIMARY KEY,
    PER_ID NUMBER CONSTRAINT FK_STUDENT_PER_ID REFERENCES TEST_PERSON(PER_ID) NULL,
    CLASS VARCHAR2(30) NULL,
    ROLE VARCHAR2(30) NULL
);
insert into TEST_STUDENT values(3,1,'class3','monitor');
insert into TEST_STUDENT values(4,2,'class9','leader');
insert into TEST_STUDENT values(5,1,'class9','normal');
------------------------------------------------------------------------------ TEST_TEACHER
create table TEST_TEACHER(
    TEA_ID NUMBER PRIMARY KEY,
    PER_ID NUMBER CONSTRAINT FK_TEACHER_PER_ID REFERENCES TEST_PERSON(PER_ID) NULL,
    CLASS VARCHAR2(30) NULL,
    ROLE VARCHAR2(30) NULL
);
insert into TEST_TEACHER values(3,1,'class3','monitor');
insert into TEST_TEACHER values(4,2,'class9','leader');
insert into TEST_TEACHER values(5,1,'class9','normal');
------------------------------------------------------------------------------ SYS_LOG
create table SYS_LOG(
    LOG_ID NUMBER PRIMARY KEY,
    EXCEPTION VARCHAR2(30) NULL
);


commit;
