// // https://github.com/pstanoev/simple-svelte-autocomplete/issues/205#issuecomment-1960396289
declare module 'simple-svelte-autocomplete' {
	import { SvelteComponent } from 'svelte';
	import { HTMLAttributes } from 'svelte/elements';

	export interface AutoCompleteAttributes<T> extends HTMLAttributes<HTMLDivElement> {
		autocompleteOffValue?: string;
		className?: string;
		cleanUserText?: boolean;
		closeOnBlur?: boolean;
		create?: boolean;
		createText?: string;
		delay?: number;
		disabled?: boolean;
		dropdownClassName?: string;
		flag?: boolean;
		hideArrow?: boolean;
		highlightedItem?: T;
		html5autocomplete?: boolean;
		ignoreAccents?: boolean;
		inputClassName?: string;
		inputId?: string;
		items?: T[];
		keywordsFieldName?: string;
		labelFieldName?: string;
		localFiltering?: boolean;
		localSorting?: boolean;
		lock?: boolean;
		lowercaseKeywords?: boolean;
		matchAllKeywords?: boolean;
		maxItemsToShowInList?: number;
		minCharactersToSearch?: number;
		moreItemsText?: string;
		multiple?: boolean;
		name?: string;
		noInputClassName?: boolean;
		noInputStyles?: boolean;
		noResultsText?: string;
		orderableSection?: boolean;
		placeholder?: string;
		readonly?: boolean;
		required?: boolean;
		selectFirstIfEmpty?: boolean;
		selectName?: string;
		selectedItem?: T;
		showClear?: boolean;
		showLoadingIndicator?: boolean;
		sortByMatchedKeywords?: boolean;
		tabIndex?: number;
		value?: T;
		valueFieldName?: string;
	}

	export interface AutoCompleteFunctions<T> {
		itemFilterFunction?: (item: T, keywords: string) => boolean;
		itemSortFunction?: (item1: T, item2: T, keywords: string) => number;
		keywordsCleanFunction?: (keywords: string) => string;
		keywordsFunction?: (item: T) => string;
		labelFunction?: (item: T) => string;
		searchFunction?: (keyword: string, maxItemsToShowInList: number) => Promise<T[]> | boolean;
		textCleanFunction?: (string) => string;
		valueFunction?: (a: T) => string;
	}

	export interface AutoCompleteCallbacks<T> {
		beforeChange?: (oldSelectedItem: T, newSelectedItem: T) => boolean;
		onChange?: (newSelectedItem: T) => void;
		onFocus?: () => void;
		onBlur?: () => void;
		onCreate?: (text: string) => void;
	}

	export interface AutoCompleteSlots<T> {
		item: { item: T; label: string };
		'no-results': null;
		loading: { loadingText: string };
		tag: null;
		'dropdown-header': { nbItems: number; maxItemsToShowInList: number };
		'dropdown-footer': { nbItems: number; maxItemsToShowInList: number };
	}

	export interface AutoCompleteProps<T>
		extends AutoCompleteAttributes<T>,
			AutoCompleteCallbacks<T>,
			AutoCompleteFunctions<T> {}

	export default class AutoComplete<T> extends SvelteComponent<
		AutoCompleteProps<T>,
		undefined,
		AutoCompleteSlots<T>
	> {}
}
