#!/usr/bin/env node

const keys = require('./keys.json');

const getPocket = require('pocket-api');

const pocket = new getPocket(keys.consumerKey);
pocket.setAccessToken(keys.accessToken);

async function fetch(offset = 0) {
	const articles = await pocket.getArticles({
		state: 'unread',
		offset,
		detailType: 'simple',
	});
	return Object.values(articles.list);
}

(async function() {
	let articles = [];

	for (let offset = 0; ; offset += 30) {
		const res = await fetch(offset);
		articles = articles.concat(res);

		process.stderr.clearLine(0);
		process.stderr.cursorTo(0);
		process.stderr.write(`${articles.length} articles fetched`);

		if (res < 30) {
			break;
		}
	}

	console.log(JSON.stringify(articles));
})().catch(e => console.error(e));
