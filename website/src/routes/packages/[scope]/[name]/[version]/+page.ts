import { error, redirect } from '@sveltejs/kit';
import type { PageLoad } from './$types';
import { extract } from 'tar-stream';
import { inflate } from 'pako';
import { parse } from 'yaml';

export const ssr = false;

type Dependencies = ({ name: string; version: string } | { repo: string; rev: string })[];

const parseAuthor = (author: string) => {
	const authorRegex =
		/^(?<name>.+?)(?:\s*<(?<email>.+?)>)?(?:\s*\((?<url>.+?)\))?(?:\s*<(?<email2>.+?)>)?(?:\s*\((?<url2>.+?)\))?$/;
	const { groups } = author.match(authorRegex) ?? {};
	return {
		name: groups?.name ?? author,
		email: groups?.email ?? groups?.email2,
		url: groups?.url ?? groups?.url2
	};
};

export const load: PageLoad = async ({ params, fetch }) => {
	const res = await fetch(
		`${import.meta.env.VITE_API_URL}/v0/packages/${params.scope}/${params.name}/${params.version}`
	);

	if (res.status === 404) {
		error(res.status, 'Package not found');
	} else if (!res.ok) {
		error(res.status, await res.text());
	}

	const body = await res.arrayBuffer();

	const extractStream = extract();
	extractStream.end(inflate(body));

	let manifestBuffer, readmeBuffer;

	for await (const entry of extractStream) {
		const read = () => {
			return new Promise<Uint8Array>((resolve, reject) => {
				const chunks: number[] = [];
				entry.on('data', (chunk: Uint8Array) => {
					chunks.push(...chunk);
				});
				entry.on('end', () => {
					resolve(new Uint8Array(chunks));
				});
				entry.on('error', reject);
			});
		};

		switch (entry.header.name.toLowerCase()) {
			case 'pesde.yaml': {
				manifestBuffer = await read();
				break;
			}
			case 'readme.md':
			case 'readme.txt':
			case 'readme': {
				readmeBuffer = await read();
				break;
			}
		}

		entry.resume();
	}

	if (!manifestBuffer) {
		error(500, 'Package is missing pesde.yaml');
	}

	const textDecoder = new TextDecoder();

	const manifest = textDecoder.decode(manifestBuffer);
	const parsed = parse(manifest, {
		customTags: [
			{
				tag: '!roblox',
				collection: 'map'
			}
		]
	}) as {
		version: string;
		authors?: string[];
		description?: string;
		license?: string;
		repository?: string;
		realm?: string;
		dependencies?: Dependencies;
		peer_dependencies?: Dependencies;
		exports?: { lib?: string; bin?: string };
	};

	if (params.version.toLowerCase() === 'latest') {
		redirect(302, `/packages/${params.scope}/${params.name}/${parsed.version}`);
	}

	const readme = readmeBuffer ? textDecoder.decode(readmeBuffer) : null;

	const versionsRes = await fetch(
		`${import.meta.env.VITE_API_URL}/v0/packages/${params.scope}/${params.name}/versions`
	);

	if (!versionsRes.ok) {
		error(versionsRes.status, await versionsRes.text());
	}

	const versions = (await versionsRes.json()) as [string, number][];

	return {
		scope: params.scope,
		name: params.name,
		version: parsed.version,
		versions: versions.map(([version]) => version),
		publishedAt: new Date(
			(versions.find(([version]) => version === parsed.version)?.[1] ?? 0) * 1000
		),
		authors: parsed.authors?.map(parseAuthor),
		description: parsed.description,
		license: parsed.license,
		readme,
		repository: parsed.repository,
		realm: parsed.realm,
		dependencies: parsed.dependencies,
		peerDependencies: parsed.peer_dependencies,
		exports: {
			lib: !!parsed.exports?.lib,
			bin: !!parsed.exports?.bin
		}
	};
};
