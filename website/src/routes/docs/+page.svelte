<script lang="ts">
	import Codeblock from '$lib/Codeblock.svelte';
	import Note from './Note.svelte';
</script>

<svelte:head>
	<title>pesde documentation</title>
	<meta content="pesde documentation" property="og:title" />
	<meta content="Documentation about using pesde" name="description" />
	<meta content="Documentation about using pesde" property="og:description" />
</svelte:head>

<div class="max-w-prose">
	<h1>Using pesde</h1>

	<section>
		<h2>Initializing a package</h2>
		<p>
			Even if you're not making a package, but something else such as a game, you will still need to
			initialize package.
		</p>
		<Codeblock code="pesde init" />
		<p>This will prompt you questions, after which it will create a pesde.yaml file.</p>
		<Note>
			If you are using pesde with the `wally` feature enabled (true on releases from the GitHub
			repository) then you can use <Codeblock code="pesde convert" /> to convert your wally.toml file
			to pesde.yaml. This will leave you with an empty default index, so you will need to add a URL (such
			as the default `<a href="https://github.com/daimond113/pesde-index"
				>https://github.com/daimond113/pesde-index</a
			>`) yourself.
		</Note>
	</section>

	<section>
		<h2>Adding dependencies</h2>
		<p>
			You can use the `add` command to add dependencies to your project. With the `wally` feature
			enabled, you can add Wally dependencies.
		</p>
		<p>
			If you are making a package, you can use the `--peer` argument to add a package as a peer
			dependency. Peer dependencies are not installed when the package is installed, but are
			required to be installed by the user of the package. This is useful for things like framework
			plugins.
		</p>
		<p>
			If you want to declare the dependency as server or development only, you can use the `--realm
			server` or `--realm development` arguments respectively. The `shared` realm is the default.
		</p>
		<Codeblock
			code="pesde add --realm server SCOPE/NAME@VERSION
pesde add --realm development wally#SCOPE/NAME@VERSION # for Wally packages"
		/>
	</section>

	<section>
		<h2>Overriding dependencies</h2>
		<p>
			Dependency overrides allow you to use a different version of a dependency than the one
			specified in the package. This is useful for sharing 1 version of a dependency.
		</p>
		<p>
			Dependency overrides use the keys in the format of desired names separated with `>`, and
			optionally other paths separated with `,`. The value is a dependency specifier.
		</p>
		<Note class="mb-4">
			Dependency overrides do not have a command. You will need to edit the pesde.yaml file
			yourself.
		</Note>
		<Codeblock
			lang="yaml"
			code="overrides:
    DESIRED_NAME>DEPENDENCY_DESIRED_NAME,DESIRED_NAME_2>DEPENDENCY_DESIRED_NAME_2:
        name: SCOPE/NAME
        version: VERSION"
		/>
	</section>

	<section>
		<h2>Removing dependencies</h2>
		<p>You can use the `remove` command to remove dependencies from your project.</p>
		<Codeblock
			code="pesde remove SCOPE/NAME@VERSION
pesde remove wally#SCOPE/NAME@VERSION"
		/>
	</section>

	<section>
		<h2>Outdated dependencies</h2>
		<p>
			You can list outdated dependencies with the `outdated` command. This will list all
			dependencies that have a newer version available.
		</p>
		<Note class="mb-4">
			This command only supports pesde registries, so neither Git nor Wally dependencies will be
			listed.
		</Note>
		<Codeblock code="pesde outdated" />
	</section>

	<section>
		<h2>Installing a project</h2>
		<p>The `install` command will install all dependencies of a project.</p>
		<p>
			You can use the `--locked` argument to skip resolving and read the dependencies from the
			lockfile. If any changes were made from the time the lockfile was generated this will error.
		</p>
		<Codeblock code="pesde install" />
	</section>

	<section>
		<h2>Running a bin dependency</h2>
		<p>
			Dependencies may export a bin script. You can run this script with the `run` command. The
			script will be executed with Lune. You can use the `--` argument to pass arguments to the
			script.
		</p>
		<Note class="mb-4">
			This does <b>not</b> support Wally dependencies.
		</Note>
		<Codeblock code="pesde run SCOPE/NAME -- -arg" />
	</section>

	<section>
		<h2>Patching dependencies</h2>
		<p>
			You can use the `patch` command to patch a dependency. This will output a directory in which
			you can edit the dependency. After you are done, run the `patch-commit` command with the
			directory as an argument to commit the changes.
		</p>
		<Codeblock
			code="pesde patch SCOPE/NAME@VERSION
pesde patch-commit DIRECTORY"
		/>
	</section>

	<section>
		<h2>Publishing a package</h2>
		<p>
			You can publish a package with the `publish` command. This will upload the package to the
			registry. This will publish to the `default` index.
		</p>
		<Note class="mb-4"
			>The official pesde registry does not support publishing packages with Wally or Git
			dependencies. Dependency overrides and patches of your package as a dependency will be
			ignored.</Note
		>
		<Codeblock code="pesde publish" />
	</section>

	<section>
		<h2>Searching for packages</h2>
		<p>
			You can search for packages with the `search` command. This will list all packages that match
			the query. It will search by name and description.
		</p>
		<Codeblock code="pesde search QUERY" />
	</section>

	<section>
		<h2>Manifest format cheat sheet</h2>
		<p>
			Here is a cheat sheet for the manifest format. This is the format of the pesde.yaml file. The
			`name` and `version` fields are required. All other fields are optional.
		</p>
		<p>A description of each type:</p>
		<ul>
			<li>PACKAGE_NAME: either a STANDARD_PACKAGE_NAME or WALLY_PACKAGE_NAME</li>
			<li>STANDARD_PACKAGE_NAME: refers to a package name used by pesde</li>
			<li>
				WALLY_PACKAGE_NAME: refers to a package name used by Wally. This will usually be prefixed
				with `wally#`, although not required when this rather than `PACKAGE_NAME` is the type
			</li>
			<li>VERSION: a semver version specifier</li>
			<li>VERSION_REQ: a semver version requirement</li>
			<li>REALM: one of `shared`, `server`, or `development`</li>
			<li>COMMAND: a command to run</li>
			<li>
				DEPENDENCY_SPECIFIER: one of REGISTRY_DEPENDENCY_SPECIFIER, GIT_DEPENDENCY_SPECIFIER,
				WALLY_DEPENDENCY_SPECIFIER
			</li>
			<li>
				REGISTRY_DEPENDENCY_SPECIFIER: an object with the following structure:
				<Codeblock
					lang="yaml"
					code="name: STANDARD_PACKAGE_NAME
version: VERSION_REQ
# OPTIONAL (name in the `indices` field)
index: STRING"
				/>
			</li>
			<li>
				GIT_DEPENDENCY_SPECIFIER: an object with the following structure:
				<Codeblock
					lang="yaml"
					code="repo: URL
rev: STRING"
				/>
			</li>
			<li>
				WALLY_DEPENDENCY_SPECIFIER: an object with the following structure:
				<Codeblock
					lang="yaml"
					code="wally: WALLY_PACKAGE_NAME
version: VERSION_REQ
index_url: URL"
				/>
			</li>
		</ul>
		<Codeblock
			lang="yaml"
			code="name: PACKAGE_NAME
version: VERSION
description: STRING
license: STRING
authors: STRING[]
repository: URL
exports:
    lib: PATH
    bin: PATH
path_style: !roblox
    place: &lbrace;
        REALM: STRING
    &rbrace;
private: BOOL
realm: REALM
indices:
    STRING: URL
# WALLY FEATURE ONLY
sourcemap_generator: COMMAND
overrides:
    OVERRIDE_KEY: DEPENDENCY_SPECIFIER

dependencies: DEPENDENCY_SPECIFIER[]
peer_dependencies: DEPENDENCY_SPECIFIER[]"
		/>
		<p>
			If the realm field is not specified, it will default to `shared`. If it is another value, and
			the package is to be installed in a different realm, pesde will error.
		</p>
		<p>
			The sourcemap generator command is only used for Wally and Git packages. It will be ran in a
			package's directory, and must output a sourcemap file. This is used to generate a sourcemap
			for the package so that types may be found and re-exported.
		</p>
	</section>
</div>
