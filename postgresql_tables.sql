CREATE TABLE comments (
    id integer NOT NULL,
    comment_id character varying(24) NOT NULL,
    body text,
    author character varying(200) NOT NULL,
    utc timestamp with time zone NOT NULL,
    controversiality_1 boolean DEFAULT false,
    controversiality_2 boolean DEFAULT false,
    controversiality_3 boolean DEFAULT false,
    score_1 smallint,
    score_2 smallint,
    score_3 smallint,
    score_4 smallint,
    score_5 smallint,
    score_6 smallint,
    post_id character varying(50) NOT NULL,
    controversiality_4 boolean DEFAULT false,
    controversiality_5 boolean DEFAULT false,
    controversiality_6 boolean DEFAULT false
);

CREATE TABLE posts (
    id integer NOT NULL,
    post_id character varying(50) NOT NULL,
    title text NOT NULL,
    selftext text,
    author character varying(200) NOT NULL,
    utc timestamp with time zone NOT NULL,
    preview text,
    ratio_1 double precision,
    ratio_2 double precision,
    ratio_3 double precision,
    score_1 smallint,
    score_2 smallint,
    score_3 smallint,
    score_4 smallint,
    score_5 smallint,
    score_6 smallint,
    subreddit text,
    inserted_utc timestamp with time zone,
    ratio_4 double precision,
    ratio_5 double precision,
    ratio_6 double precision,
    subreddit_size integer
);

CREATE TABLE updates (
    id integer NOT NULL,
    post_id character varying(50) NOT NULL,
    utc timestamp with time zone NOT NULL,
    init timestamp with time zone NOT NULL,
    stage smallint,
    link text,
    update_time timestamp with time zone NOT NULL
);
