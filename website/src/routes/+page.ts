import { error } from '@sveltejs/kit';
import type { PageLoad } from './$types';

export const ssr = false;

export const load: PageLoad = async ({ fetch }) => {
	const latestRes = await fetch(`${import.meta.env.VITE_API_URL}/v0/search`);

	if (!latestRes.ok) {
		error(latestRes.status, await latestRes.text());
	}

	const latest = (await latestRes.json()) as {
		name: string;
		version: string;
		description?: string;
		published_at: string;
	}[];

	return {
		latest: latest.map((pkg) => ({
			...pkg,
			published_at: new Date(parseInt(pkg.published_at) * 1000)
		}))
	};
};
